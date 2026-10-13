use std::collections::HashMap;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::logic::{
    caption_mask_usable_after_flush, composite_premultiplied_bgra, wheel_boundary_angle_degrees,
    wheel_wedge_outline,
};
use image::RgbaImage;
use pingloom_core::model::{PingId, WheelKind};
use pingloom_core::settings::Settings;
use pingloom_core::{geometry::pick_slice, Point};
use webp_animation::Decoder;
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::BLENDFUNCTION;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GdiFlush,
    GetDC, GetStockObject, ReleaseDC, SelectObject, SetBkMode, SetTextColor, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DEFAULT_GUI_FONT, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, FW_SEMIBOLD, HBITMAP, HDC, HFONT, HGDIOBJ, TRANSPARENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, RegisterClassW, SetWindowPos, ShowWindow, UpdateLayeredWindow,
    CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
    SW_SHOWNOACTIVATE, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use super::assets::ASSETS;

const WHEEL_INNER: f64 = 86.0;
const WHEEL_OUTER: f64 = 220.0;
const MAX_PINGS: usize = 60;
const EMOTE_LIFETIME: Duration = Duration::from_secs(3);
const MAX_ANIMATION_BYTES: usize = 128 * 1024 * 1024;
const MAX_ANIMATION_FRAMES: usize = 300;

#[derive(Clone)]
pub struct AnimatedFrame {
    pub at_ms: u32,
    image: RgbaImage,
}

struct PingFx {
    id: PingId,
    point: Point,
    started: Instant,
}

struct EmoteFx {
    slug: String,
    point: Point,
    size: u16,
    started: Instant,
    still: RgbaImage,
    frames: Option<Vec<AnimatedFrame>>,
}

struct CaptionMask {
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    pixels: *mut u8,
    width: i32,
    height: i32,
    line_height: i32,
    usable: bool,
}

struct WheelFx {
    kind: WheelKind,
    origin: Point,
    pointer: Point,
    scale: f64,
    hot: Option<usize>,
    caption_font: HFONT,
    caption_mask: Option<CaptionMask>,
}

impl CaptionMask {
    unsafe fn new(template: HDC, scale: f64) -> Option<Self> {
        let width = ((WHEEL_INNER * 2.0 - 12.0) * scale).round().max(1.0) as i32;
        let line_height = (16.0 * scale).round().max(1.0) as i32;
        let height = line_height.checked_mul(7)?;
        let bytes = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(4)?;
        let image_size = u32::try_from(bytes).ok()?;
        let dc = CreateCompatibleDC(template);
        if dc.is_null() {
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: image_size,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [zeroed()],
        };
        let mut raw_pixels = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut raw_pixels, null_mut(), 0);
        if bitmap.is_null() || raw_pixels.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap as HGDIOBJ);
            }
            DeleteDC(dc);
            return None;
        }
        let old_bitmap = SelectObject(dc, bitmap as HGDIOBJ);
        if old_bitmap.is_null() {
            DeleteObject(bitmap as HGDIOBJ);
            DeleteDC(dc);
            return None;
        }
        Some(Self {
            dc,
            bitmap,
            old_bitmap,
            pixels: raw_pixels.cast(),
            width,
            height,
            line_height,
            usable: true,
        })
    }
}

impl Drop for CaptionMask {
    fn drop(&mut self) {
        unsafe {
            GdiFlush();
            SelectObject(self.dc, self.old_bitmap);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
        }
    }
}

pub struct Overlay {
    hwnd: HWND,
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    pixels: *mut u8,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    image_cache: HashMap<String, Arc<RgbaImage>>,
    pings: Vec<PingFx>,
    emote: Option<EmoteFx>,
    wheel: Option<WheelFx>,
}

pub fn register(instance: HINSTANCE) -> Result<(), String> {
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: w!("pingloom-overlay"),
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err("could not register overlay window class".into());
    }
    Ok(())
}

unsafe extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wp: usize, lp: LPARAM) -> isize {
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

impl Overlay {
    pub fn new(instance: HINSTANCE) -> Result<Self, String> {
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if width <= 0 || height <= 0 || (width as u64) * (height as u64) > 100_000_000 {
                return Err("invalid virtual desktop dimensions".into());
            }
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("pingloom-overlay"),
                w!("Pingloom overlay"),
                WS_POPUP,
                left,
                top,
                width,
                height,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if hwnd.is_null() {
                return Err("could not create click-through overlay window".into());
            }
            let screen_dc = GetDC(null_mut());
            let dc = CreateCompatibleDC(screen_dc);
            ReleaseDC(null_mut(), screen_dc);
            if dc.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                return Err("could not create overlay drawing context".into());
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: (width as u32)
                        .saturating_mul(height as u32)
                        .saturating_mul(4),
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [zeroed()],
            };
            let mut raw_pixels = null_mut();
            let bitmap =
                CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut raw_pixels, null_mut(), 0);
            if bitmap.is_null() || raw_pixels.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap as HGDIOBJ);
                }
                DeleteDC(dc);
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                return Err("could not allocate overlay surface".into());
            }
            let old_bitmap = SelectObject(dc, bitmap as HGDIOBJ);
            let result = Self {
                hwnd,
                dc,
                bitmap,
                old_bitmap,
                pixels: raw_pixels.cast(),
                width,
                height,
                left,
                top,
                image_cache: HashMap::new(),
                pings: Vec::new(),
                emote: None,
                wheel: None,
            };
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                left,
                top,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            Ok(result)
        }
    }

    pub fn wheel_open(&mut self, kind: WheelKind, origin: Point, scale: f64) {
        self.wheel_close();
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let face: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
        let caption_font = unsafe {
            CreateFontW(
                -((14.0 * scale).round() as i32).max(1),
                0,
                0,
                0,
                FW_SEMIBOLD as i32,
                0,
                0,
                0,
                windows_sys::Win32::Graphics::Gdi::DEFAULT_CHARSET as u32,
                0,
                0,
                0,
                0,
                face.as_ptr(),
            )
        };
        self.wheel = Some(WheelFx {
            kind,
            origin,
            pointer: origin,
            scale,
            hot: None,
            caption_font,
            caption_mask: unsafe { CaptionMask::new(self.dc, scale) },
        });
    }

    pub fn wheel_move(&mut self, point: Point) -> bool {
        let Some(wheel) = &mut self.wheel else {
            return false;
        };
        wheel.pointer = point;
        let dx = i64::from(point.x) - i64::from(wheel.origin.x);
        let dy = i64::from(point.y) - i64::from(wheel.origin.y);
        let next = pick_slice(dx as f64, dy as f64, WHEEL_INNER * wheel.scale);
        if wheel.hot == next {
            return false;
        }
        wheel.hot = next;
        next.is_some()
    }

    pub fn wheel_origin(&self) -> Option<(WheelKind, Point)> {
        self.wheel.as_ref().map(|wheel| (wheel.kind, wheel.origin))
    }

    pub fn wheel_close(&mut self) {
        if let Some(wheel) = self.wheel.take() {
            if !wheel.caption_font.is_null() {
                unsafe { DeleteObject(wheel.caption_font as HGDIOBJ) };
            }
        }
    }

    pub fn wheel_close_if(&mut self, kind: WheelKind, origin: Point) {
        if self
            .wheel
            .as_ref()
            .is_some_and(|wheel| wheel.kind == kind && wheel.origin == origin)
        {
            self.wheel_close();
        }
    }

    pub fn spawn_ping(&mut self, id: PingId, point: Point) {
        if self.pings.len() >= MAX_PINGS {
            self.pings.remove(0);
        }
        self.pings.push(PingFx {
            id,
            point,
            started: Instant::now(),
        });
    }

    pub fn spawn_emote(&mut self, slug: &str, point: Point, size: u16) -> Option<String> {
        let bytes = ASSETS.get_file(format!("emotes/{slug}.png"))?.contents();
        let still = image::load_from_memory(bytes).ok()?.into_rgba8();
        self.emote = Some(EmoteFx {
            slug: slug.to_owned(),
            point,
            size,
            started: Instant::now(),
            still,
            frames: None,
        });
        Some(slug.to_owned())
    }

    pub fn set_emote_frames(&mut self, slug: &str, frames: Vec<AnimatedFrame>) {
        if let Some(emote) = &mut self.emote {
            if emote.slug == slug {
                emote.frames = Some(frames);
            }
        }
    }

    pub fn tick(&mut self, settings: &Settings) {
        let now = Instant::now();
        let duration = Duration::from_secs_f32(settings.ping_duration_s);
        self.pings
            .retain(|ping| now.duration_since(ping.started) < duration);
        if self
            .emote
            .as_ref()
            .is_some_and(|emote| now.duration_since(emote.started) >= EMOTE_LIFETIME)
        {
            self.emote = None;
        }
        if self.wheel.is_none() && self.pings.is_empty() && self.emote.is_none() {
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
            return;
        }

        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let len = (self.width as usize) * (self.height as usize) * 4;
            std::ptr::write_bytes(self.pixels, 0, len);
            let mut canvas = Canvas {
                pixels: std::slice::from_raw_parts_mut(self.pixels, len),
                width: self.width as usize,
                height: self.height as usize,
                left: self.left,
                top: self.top,
            };
            if let Some(wheel) = &mut self.wheel {
                let (items, names) = match wheel.kind {
                    WheelKind::Ping => (
                        settings
                            .wheel
                            .iter()
                            .map(|id| id.asset().icon.to_owned())
                            .collect::<Vec<_>>(),
                        settings
                            .wheel
                            .iter()
                            .map(|id| id.asset().name.to_owned())
                            .collect::<Vec<_>>(),
                    ),
                    WheelKind::Emote => (
                        settings.emote_wheel.to_vec(),
                        settings
                            .emote_wheel
                            .iter()
                            .map(|slug| readable_emote_name(slug))
                            .collect(),
                    ),
                };
                canvas.draw_wheel(wheel, &items, &mut self.image_cache);
                let selected = wheel
                    .hot
                    .and_then(|slot| names.get(slot))
                    .map(String::as_str);
                draw_wheel_caption(&mut canvas, wheel, selected);
            }
            for ping in &self.pings {
                let age = now.duration_since(ping.started).as_secs_f64();
                canvas.draw_ping(ping, settings, age, &mut self.image_cache);
            }
            if let Some(emote) = &self.emote {
                let elapsed = now.duration_since(emote.started).as_millis() as u32;
                let frame = emote
                    .frames
                    .as_ref()
                    .and_then(|frames| frames.iter().rev().find(|frame| frame.at_ms <= elapsed));
                let image = frame.map_or(&emote.still, |frame| &frame.image);
                let life = now.duration_since(emote.started).as_secs_f64();
                canvas.draw_emote(emote, image, life);
            }
            let screen_dc = GetDC(null_mut());
            let destination = POINT {
                x: self.left,
                y: self.top,
            };
            let size = SIZE {
                cx: self.width,
                cy: self.height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: 0,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 1,
            };
            UpdateLayeredWindow(
                self.hwnd,
                screen_dc,
                &destination,
                &size,
                self.dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );
            ReleaseDC(null_mut(), screen_dc);
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.wheel_close();
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
            SelectObject(self.dc, self.old_bitmap);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
        }
    }
}

unsafe fn draw_wheel_caption(canvas: &mut Canvas<'_>, wheel: &mut WheelFx, selected: Option<&str>) {
    let Some(mask) = wheel.caption_mask.as_mut() else {
        return;
    };
    if !mask.usable {
        return;
    }
    let bytes = mask.width as usize * mask.height as usize * 4;
    std::ptr::write_bytes(mask.pixels, 0, bytes);
    let font = if wheel.caption_font.is_null() {
        GetStockObject(DEFAULT_GUI_FONT)
    } else {
        wheel.caption_font as HGDIOBJ
    };
    let old_font = SelectObject(mask.dc, font);
    let old_mode = SetBkMode(mask.dc, TRANSPARENT as i32);
    let old_color = SetTextColor(mask.dc, 0x00f8_f8f8);
    let (title, instruction) = selected.map_or(
        (
            "Choose a slice".to_owned(),
            "Move over an action".to_owned(),
        ),
        |name| (format!("Selected: {name}"), "Release to choose".to_owned()),
    );
    let title = title
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let instruction = instruction
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let cancel = "CANCEL\0".encode_utf16().collect::<Vec<_>>();
    let escape = "Esc / right-click\0".encode_utf16().collect::<Vec<_>>();
    for (text, line) in [
        (title.as_ptr(), 0),
        (instruction.as_ptr(), 1),
        (cancel.as_ptr(), 5),
        (escape.as_ptr(), 6),
    ] {
        let top = line * mask.line_height;
        let mut rect = windows_sys::Win32::Foundation::RECT {
            left: 0,
            top,
            right: mask.width,
            bottom: top + mask.line_height,
        };
        DrawTextW(
            mask.dc,
            text,
            -1,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_NOPREFIX | DT_SINGLELINE,
        );
    }
    SetTextColor(mask.dc, old_color);
    SetBkMode(mask.dc, old_mode);
    SelectObject(mask.dc, old_font);
    mask.usable = caption_mask_usable_after_flush(mask.usable, GdiFlush() != 0);
    if !mask.usable {
        return;
    }

    let left = wheel.origin.x - mask.width / 2;
    let top = wheel.origin.y - mask.line_height * 4;
    for y in 0..mask.height {
        for x in 0..mask.width {
            let index = (y as usize * mask.width as usize + x as usize) * 4;
            let coverage = (*mask.pixels.add(index))
                .max(*mask.pixels.add(index + 1))
                .max(*mask.pixels.add(index + 2));
            if coverage != 0 {
                canvas.point(left + x, top + y, [248, 248, 248, coverage]);
            }
        }
    }
}

fn readable_emote_name(slug: &str) -> String {
    slug.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn decode_emote(slug: &str) -> Option<Vec<AnimatedFrame>> {
    let bytes = ASSETS.get_file(format!("emotes/{slug}.webp"))?.contents();
    let decoder = Decoder::new(bytes).ok()?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return None;
    }
    let mut frames = Vec::new();
    let mut decoded_bytes = 0usize;
    for frame in decoder {
        if frames.len() >= MAX_ANIMATION_FRAMES {
            return None;
        }
        let (frame_width, frame_height) = frame.dimensions();
        let size = (frame_width as usize)
            .checked_mul(frame_height as usize)?
            .checked_mul(4)?;
        decoded_bytes = decoded_bytes.checked_add(size)?;
        if decoded_bytes > MAX_ANIMATION_BYTES || frame.data().len() != size {
            return None;
        }
        let image = RgbaImage::from_raw(frame_width, frame_height, frame.data().to_vec())?;
        frames.push(AnimatedFrame {
            at_ms: frame.timestamp().max(0) as u32,
            image,
        });
    }
    (!frames.is_empty()).then_some(frames)
}

struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: usize,
    height: usize,
    left: i32,
    top: i32,
}

impl Canvas<'_> {
    fn point(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        let x = x - self.left;
        let y = y - self.top;
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let index = (y as usize * self.width + x as usize) * 4;
        let destination = [
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        ];
        let blended = composite_premultiplied_bgra(destination, rgba);
        self.pixels[index..index + 4].copy_from_slice(&blended);
    }

    fn circle(&mut self, center: Point, radius: f64, thickness: f64, color: [u8; 4]) {
        let outer = radius + thickness / 2.0;
        let inner = (radius - thickness / 2.0).max(0.0);
        for dy in -(outer.ceil() as i32)..=outer.ceil() as i32 {
            for dx in -(outer.ceil() as i32)..=outer.ceil() as i32 {
                let distance = f64::from(dx).hypot(f64::from(dy));
                if distance >= inner && distance <= outer {
                    self.point(center.x + dx, center.y + dy, color);
                }
            }
        }
    }

    fn filled_circle(&mut self, center: Point, radius: i32, color: [u8; 4]) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if f64::from(dx).hypot(f64::from(dy)) <= f64::from(radius) {
                    self.point(center.x + dx, center.y + dy, color);
                }
            }
        }
    }

    fn line(&mut self, start: Point, end: Point, thickness: i32, color: [u8; 4]) {
        let (mut x, mut y) = (start.x, start.y);
        let dx = (end.x - start.x).abs();
        let sx = if start.x < end.x { 1 } else { -1 };
        let dy = -(end.y - start.y).abs();
        let sy = if start.y < end.y { 1 } else { -1 };
        let mut error = dx + dy;
        let radius = (thickness - 1).max(0) / 2;
        loop {
            for oy in -radius..=radius {
                for ox in -radius..=radius {
                    self.point(x + ox, y + oy, color);
                }
            }
            if x == end.x && y == end.y {
                break;
            }
            let twice = error * 2;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }

    fn wedge_outline(&mut self, center: Point, inner: f64, outer: f64, slot: usize, scale: f64) {
        let Some(points) = wheel_wedge_outline(slot, inner, outer) else {
            return;
        };
        let thickness = (3.0 * scale).round().max(2.0) as i32;
        let points: Vec<Point> = points
            .into_iter()
            .map(|(x, y)| Point::new(center.x + x as i32, center.y + y as i32))
            .collect();
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], thickness, [255, 245, 208, 255]);
        }
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            self.line(*last, *first, thickness, [255, 245, 208, 255]);
        }
    }

    fn ellipse(&mut self, center: Point, rx: f64, ry: f64, color: [u8; 4]) {
        for dy in -(ry.ceil() as i32)..=ry.ceil() as i32 {
            for dx in -(rx.ceil() as i32)..=rx.ceil() as i32 {
                let x = f64::from(dx) / rx;
                let y = f64::from(dy) / ry;
                if x * x + y * y <= 1.0 {
                    self.point(center.x + dx, center.y + dy, color);
                }
            }
        }
    }

    fn image(&mut self, image: &RgbaImage, x: i32, y: i32, size: u32, opacity: f64) {
        if size == 0 || image.width() == 0 || image.height() == 0 {
            return;
        }
        let height =
            (u64::from(size) * u64::from(image.height()) / u64::from(image.width())) as u32;
        let size = size as usize;
        let height = height.max(1) as usize;
        for dy in 0..height {
            for dx in 0..size {
                let sx = dx * image.width() as usize / size;
                let sy = dy * image.height() as usize / height;
                let pixel = image.get_pixel(sx as u32, sy as u32).0;
                self.point(
                    x + dx as i32,
                    y + dy as i32,
                    [
                        pixel[0],
                        pixel[1],
                        pixel[2],
                        (f64::from(pixel[3]) * opacity.clamp(0.0, 1.0)) as u8,
                    ],
                );
            }
        }
    }

    fn asset(
        &mut self,
        cache: &mut HashMap<String, Arc<RgbaImage>>,
        path: &str,
    ) -> Option<Arc<RgbaImage>> {
        if let Some(image) = cache.get(path) {
            return Some(Arc::clone(image));
        }
        let image = Arc::new(
            image::load_from_memory(ASSETS.get_file(path)?.contents())
                .ok()?
                .into_rgba8(),
        );
        cache.insert(path.to_owned(), Arc::clone(&image));
        Some(image)
    }

    fn draw_wheel(
        &mut self,
        wheel: &WheelFx,
        items: &[String],
        cache: &mut HashMap<String, Arc<RgbaImage>>,
    ) {
        let scale = wheel.scale;
        let outer = WHEEL_OUTER * scale;
        let inner = WHEEL_INNER * scale;
        let extent = outer.ceil() as i32;
        for dy in -extent..=extent {
            for dx in -extent..=extent {
                let distance = f64::from(dx).hypot(f64::from(dy));
                if distance >= inner && distance <= outer {
                    let slice = pick_slice(f64::from(dx), f64::from(dy), 0.0);
                    let hot = wheel.hot == slice;
                    let color = if hot {
                        [221, 206, 154, 238]
                    } else {
                        [35, 42, 50, 218]
                    };
                    self.point(wheel.origin.x + dx, wheel.origin.y + dy, color);
                }
            }
        }
        self.circle(wheel.origin, inner, 3.0 * scale, [232, 222, 198, 245]);
        self.circle(wheel.origin, outer, 2.0 * scale, [222, 226, 232, 220]);
        for boundary in 0..8 {
            let angle = wheel_boundary_angle_degrees(boundary).to_radians();
            self.line(
                Point::new(
                    wheel.origin.x + (angle.cos() * inner) as i32,
                    wheel.origin.y + (angle.sin() * inner) as i32,
                ),
                Point::new(
                    wheel.origin.x + (angle.cos() * outer) as i32,
                    wheel.origin.y + (angle.sin() * outer) as i32,
                ),
                (1.5 * scale).round().max(1.0) as i32,
                [220, 225, 232, 180],
            );
        }
        if let Some(slot) = wheel.hot {
            self.wedge_outline(wheel.origin, inner, outer, slot, scale);
        }
        self.filled_circle(wheel.origin, inner.floor() as i32 - 2, [34, 38, 45, 255]);
        self.circle(wheel.origin, inner - 1.0, 2.5 * scale, [255, 170, 170, 255]);
        let cross = (11.0 * scale).round() as i32;
        let cross_y = wheel.origin.y - (12.0 * scale).round() as i32;
        self.line(
            Point::new(wheel.origin.x - cross, cross_y - cross),
            Point::new(wheel.origin.x + cross, cross_y + cross),
            (4.0 * scale).round().max(3.0) as i32,
            [255, 137, 137, 255],
        );
        self.line(
            Point::new(wheel.origin.x + cross, cross_y - cross),
            Point::new(wheel.origin.x - cross, cross_y + cross),
            (4.0 * scale).round().max(3.0) as i32,
            [255, 137, 137, 255],
        );
        for (slot, item) in items.iter().enumerate() {
            let angle = (slot as f64 * 45.0 - 90.0).to_radians();
            let radius = (inner + outer) / 2.0;
            let center = Point::new(
                wheel.origin.x + (angle.cos() * radius) as i32,
                wheel.origin.y + (angle.sin() * radius) as i32,
            );
            let image_name = if wheel.kind == WheelKind::Ping {
                format!("textures/{item}.png")
            } else {
                format!("emotes/{item}.png")
            };
            if let Some(icon) = self.asset(cache, &image_name) {
                let icon_size =
                    (46.0 * scale * if wheel.hot == Some(slot) { 1.18 } else { 1.0 }) as u32;
                if wheel.hot == Some(slot) {
                    self.circle(
                        center,
                        f64::from(icon_size) * 0.64,
                        2.5 * scale,
                        [255, 245, 208, 255],
                    );
                }
                self.image(
                    &icon,
                    center.x - icon_size as i32 / 2,
                    center.y - icon_size as i32 / 2,
                    icon_size,
                    1.0,
                );
            }
        }
    }

    fn draw_ping(
        &mut self,
        ping: &PingFx,
        settings: &Settings,
        age: f64,
        cache: &mut HashMap<String, Arc<RgbaImage>>,
    ) {
        let asset = ping.id.asset();
        let size = f64::from(settings.ping_size_px);
        let ping_color = color(asset.color);
        self.ellipse(
            ping.point,
            size * 0.42,
            size * 0.12,
            [ping_color[0], ping_color[1], ping_color[2], 85],
        );
        for (delay, ring_alpha) in [(0.0, 230), (0.22, 210), (1.4, 190)] {
            let t = age - delay;
            if (0.0..0.65).contains(&t) {
                let radius = size * 0.545 * (0.25 + 1.10 * t / 0.65);
                let alpha = (f64::from(ring_alpha) * (1.0 - t / 0.65)) as u8;
                self.circle(
                    ping.point,
                    radius,
                    3.0,
                    [ping_color[0], ping_color[1], ping_color[2], alpha],
                );
            }
        }
        let fade = if age > f64::from(settings.ping_duration_s) * 0.87 {
            1.0 - (age / f64::from(settings.ping_duration_s) - 0.87) / 0.13
        } else {
            1.0
        }
        .clamp(0.0, 1.0);
        let pop = if age < 0.28 {
            0.4 + 0.75 * (age / 0.28).min(1.0)
        } else {
            1.0 + 0.04 * (age * std::f64::consts::TAU / 1.6).sin()
        };
        let art_y = ping.point.y - (size * 0.73) as i32;
        for (index, art) in asset.art.iter().enumerate() {
            let Some(image) = self.asset(cache, &format!("textures/{art}.png")) else {
                continue;
            };
            let art_size = (size * pop) as u32;
            let offset = if asset.art.len() == 2 && age < 0.22 {
                let slide = (1.0 - age / 0.22).max(0.0) * size * 0.64;
                if index == 0 {
                    slide
                } else {
                    -slide
                }
            } else {
                0.0
            } as i32;
            self.image(
                &image,
                ping.point.x - art_size as i32 / 2 + offset,
                art_y,
                art_size,
                fade,
            );
        }
    }

    fn draw_emote(&mut self, emote: &EmoteFx, image: &RgbaImage, age: f64) {
        let size = f64::from(emote.size);
        let bob = (age * std::f64::consts::TAU / 1.4).sin() * 4.0;
        let pop = if age < 0.25 {
            (age / 0.25 * 1.15).min(1.15)
        } else {
            1.0
        };
        let fade = if age > 2.65 {
            ((3.0 - age) / 0.35).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let draw_size = (size * 1.25 * pop.max(0.1)) as u32;
        self.image(
            image,
            emote.point.x - draw_size as i32 / 2,
            emote.point.y - draw_size as i32 / 2 + bob as i32,
            draw_size,
            fade,
        );
    }
}

fn color(hex: &str) -> [u8; 3] {
    let hex = hex.trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).unwrap_or(0x2e8bff);
    [
        ((value >> 16) & 255) as u8,
        ((value >> 8) & 255) as u8,
        (value & 255) as u8,
    ]
}

const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;

use windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics;
