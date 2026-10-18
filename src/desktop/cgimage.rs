//! Direct AppKit presentation. Provider owns the bytes until CoreGraphics releases it.
use core_foundation::base::CFRelease;
use std::ffi::c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateWithName(name: *const c_void) -> *mut c_void;
    static kCGColorSpaceSRGB: *const c_void;
    fn CGDataProviderCreateWithData(
        info: *mut c_void,
        data: *const c_void,
        size: usize,
        release: Option<unsafe extern "C" fn(*mut c_void, *const c_void, usize)>,
    ) -> *mut c_void;
    fn CGImageCreate(
        width: usize,
        height: usize,
        bits: usize,
        pixel_bits: usize,
        stride: usize,
        space: *const c_void,
        info: u32,
        provider: *const c_void,
        decode: *const f64,
        interpolate: bool,
        intent: u32,
    ) -> *mut c_void;
}
unsafe extern "C" fn release_bytes(info: *mut c_void, _data: *const c_void, _size: usize) {
    drop(unsafe { Box::from_raw(info as *mut Vec<u8>) });
}
pub(super) fn image(frame: &image::RgbaImage) -> Result<*mut c_void, String> {
    if frame.width() == 0 || frame.height() == 0 {
        return Err("empty native frame".into());
    }
    let mut bytes = frame.as_raw().clone();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for c in &mut pixel[..3] {
            *c = ((u16::from(*c) * alpha + 127) / 255) as u8;
        }
    }
    let bytes = Box::new(bytes);
    unsafe {
        let data = bytes.as_ptr();
        let len = bytes.len();
        let info = Box::into_raw(bytes);
        let provider =
            CGDataProviderCreateWithData(info.cast(), data.cast(), len, Some(release_bytes));
        if provider.is_null() {
            drop(Box::from_raw(info));
            return Err("CGDataProvider allocation failed".into());
        }
        let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        if space.is_null() {
            CFRelease(provider);
            return Err("sRGB color space unavailable".into());
        }
        // RGBA bytes, premultiplied-last; explicit big byte order avoids host-endian ambiguity.
        let image = CGImageCreate(
            frame.width() as usize,
            frame.height() as usize,
            8,
            32,
            frame.width() as usize * 4,
            space,
            1 | (4 << 12),
            provider,
            std::ptr::null(),
            true,
            0,
        );
        CFRelease(space);
        CFRelease(provider);
        if image.is_null() {
            Err("CGImage allocation failed".into())
        } else {
            Ok(image)
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn cgimage_matches_png_roundtrip_within_one_channel() {
        use image::ImageEncoder;
        use objc2::{class, msg_send, runtime::AnyObject};
        use objc2_foundation::{NSPoint, NSRect, NSSize};
        unsafe extern "C" {
            fn CGBitmapContextCreate(
                data: *mut std::ffi::c_void,
                width: usize,
                height: usize,
                bits: usize,
                stride: usize,
                space: *const std::ffi::c_void,
                info: u32,
            ) -> *mut std::ffi::c_void;
            fn CGContextDrawImage(
                context: *mut std::ffi::c_void,
                rect: core_graphics::geometry::CGRect,
                image: *const std::ffi::c_void,
            );
        }
        let frame = image::RgbaImage::from_fn(16, 16, |x, y| {
            image::Rgba([(x * 17) as u8, (y * 17) as u8, 123, ((x + y) * 8) as u8])
        });
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(frame.as_raw(), 16, 16, image::ExtendedColorType::Rgba8)
            .unwrap();
        let data = objc2_foundation::NSData::with_bytes(&png);
        unsafe {
            let pool: *mut AnyObject = msg_send![class!(NSAutoreleasePool), new];
            let old: *mut AnyObject = msg_send![class!(NSImage), alloc];
            let old: *mut AnyObject = msg_send![old,initWithData:&*data];
            assert!(!old.is_null());
            let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(16.0, 16.0));
            let old_image: *mut std::ffi::c_void = msg_send![old,CGImageForProposedRect:&mut rect, context:std::ptr::null_mut::<AnyObject>(), hints:std::ptr::null_mut::<AnyObject>()];
            let direct = super::image(&frame).unwrap();
            let space = super::CGColorSpaceCreateWithName(super::kCGColorSpaceSRGB);
            let render = |image: *mut std::ffi::c_void| {
                let mut bytes = vec![0u8; 16 * 16 * 4];
                let context = CGBitmapContextCreate(
                    bytes.as_mut_ptr().cast(),
                    16,
                    16,
                    8,
                    64,
                    space,
                    1 | (4 << 12),
                );
                assert!(!context.is_null());
                CGContextDrawImage(
                    context,
                    core_graphics::geometry::CGRect::new(
                        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
                        &core_graphics::geometry::CGSize::new(16.0, 16.0),
                    ),
                    image,
                );
                core_foundation::base::CFRelease(context);
                bytes
            };
            let before = render(old_image);
            let after = render(direct);
            core_foundation::base::CFRelease(direct);
            core_foundation::base::CFRelease(space);
            let _: () = msg_send![old, release];
            let _: () = msg_send![pool, drain];
            let max = before
                .iter()
                .zip(after)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            assert!(max <= 1, "PNG/CGImage channel difference {max}");
        }
    }
    #[test]
    fn direct_image_keeps_dimensions_and_survives_source_drop() {
        unsafe extern "C" {
            fn CGImageGetWidth(image: *const std::ffi::c_void) -> usize;
            fn CGImageGetHeight(image: *const std::ffi::c_void) -> usize;
        }
        let frame = image::RgbaImage::from_pixel(7, 9, image::Rgba([123, 45, 67, 128]));
        let image = super::image(&frame).unwrap();
        drop(frame);
        unsafe {
            assert_eq!(CGImageGetWidth(image), 7);
            assert_eq!(CGImageGetHeight(image), 9);
            core_foundation::base::CFRelease(image);
        }
    }
}
