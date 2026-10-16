use std::collections::HashMap;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::animation::decode_emote;
pub(crate) use crate::animation::AnimatedFrame;
use crate::logic::{
    caption_mask_usable_after_flush, composite_premultiplied_bgra, effect_fade, emote_envelope,
    ping_art_motion, ring_coverage, selection_blend,
};
use image::RgbaImage;
use pingloom_core::model::{PingId, WheelKind};
use pingloom_core::settings::Settings;
use pingloom_core::{geometry::pick_slice, Point};
#[cfg(test)]
use webp_animation::Decoder;
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::BLENDFUNCTION;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, DrawTextW, GdiFlush,
    GetDC, GetStockObject, ReleaseDC, SelectObject, SetBkMode, SetTextColor, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DEFAULT_GUI_FONT, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, FW_SEMIBOLD, HBITMAP, HDC, HFONT, HGDIOBJ, TRANSPARENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, IsWindowVisible, RegisterClassW, SetWindowPos, ShowWindow,
    SystemParametersInfoW, UpdateLayeredWindow, UpdateLayeredWindowIndirect, CS_HREDRAW,
    CS_VREDRAW, HWND_TOPMOST, SPI_GETCLIENTAREAANIMATION, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA, UPDATELAYEREDWINDOWINFO, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use super::assets;

const WHEEL_INNER: f64 = pingloom_core::geometry::WHEEL_INNER_RADIUS;
const WHEEL_OUTER: f64 = 220.0;
const MAX_PINGS: usize = 60;
const EMOTE_LIFETIME: Duration = Duration::from_secs(3);

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
    animation_started: Option<Instant>,
    animation_pending: bool,
}

impl EmoteFx {
    fn complete_decode(&mut self, frames: Vec<AnimatedFrame>, ready: Instant) {
        self.animation_pending = false;
        self.started = ready;
        self.frames = Some(frames);
        self.animation_started = Some(ready);
    }

    fn image_at(&self, now: Instant, reduced_motion: bool) -> &RgbaImage {
        if !reduced_motion {
            let elapsed = self.animation_started.map_or(0, |started| {
                now.saturating_duration_since(started).as_millis() as u32
            });
            if let Some(frame) = self.frames.as_ref().and_then(|frames| {
                let elapsed = crate::animation::playback_ms(&self.slug, frames, elapsed);
                frames.iter().rev().find(|frame| frame.at_ms <= elapsed)
            }) {
                return &frame.image;
            }
        }
        &self.still
    }
}

struct CaptionMask {
    scale: f64,
    font: HFONT,
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    pixels: *mut u8,
    width: i32,
    height: i32,
    line_height: i32,
    usable: bool,
    caption: Option<String>,
}

struct WheelLayer {
    theme: crate::appearance::Theme,
    extent: i32,
    pixels: Vec<u8>,
}

impl WheelLayer {
    fn new(surface: &crate::wheel::Surface, scale: f64) -> Self {
        let extent = (crate::wheel::EXTENT * scale).ceil() as i32;
        let width = (extent * 2 + 1) as usize;
        let mut pixels = vec![0; width * width * 4];
        surface.paint(&[0.0; 8], |x, y, rgba| {
            let index = ((y + extent) as usize * width + (x + extent) as usize) * 4;
            pixels[index..index + 4].copy_from_slice(&composite_premultiplied_bgra([0; 4], rgba));
        });
        if let Some(labels) = &surface.labels {
            let offset = extent - labels.width() as i32 / 2;
            for (x, y, pixel) in labels.enumerate_pixels() {
                let index =
                    ((y as i32 + offset) as usize * width + (x as i32 + offset) as usize) * 4;
                let destination = pixels[index..index + 4].try_into().unwrap();
                pixels[index..index + 4]
                    .copy_from_slice(&composite_premultiplied_bgra(destination, pixel.0));
            }
        }
        Self {
            theme: surface.theme,
            extent,
            pixels,
        }
    }
}

struct WheelFx {
    kind: WheelKind,
    origin: Point,
    pointer: Point,
    scale: f64,
    hot: Option<usize>,
    selection: [f64; 8],
    opened: Instant,
    updated: Instant,
    caption_mask: Option<CaptionMask>,
    surface: crate::wheel::Surface,
    layer: Option<WheelLayer>,
    scaled_art: Vec<((String, u32), Arc<RgbaImage>)>,
}

impl WheelFx {
    fn advance(&mut self, now: Instant, reduced: bool) {
        let elapsed = now.saturating_duration_since(self.updated).as_secs_f64();
        self.updated = now;
        for (slot, amount) in self.selection.iter_mut().enumerate() {
            let selected = self.hot == Some(slot);
            *amount = if reduced {
                if selected {
                    1.0
                } else {
                    0.0
                }
            } else {
                selection_blend(*amount, selected, elapsed)
            };
        }
    }
    fn settled(&self, now: Instant, reduced: bool) -> bool {
        (reduced || now.saturating_duration_since(self.opened) >= Duration::from_millis(120))
            && self.selection.iter().enumerate().all(|(slot, amount)| {
                if self.hot == Some(slot) {
                    *amount == 1.0
                } else {
                    *amount <= 1e-12
                }
            })
    }
    fn icon(&mut self, path: &str, image: &RgbaImage, size: u32) -> Arc<RgbaImage> {
        crate::raster::cached_art(&mut self.scaled_art, path, image, size)
    }
}

impl CaptionMask {
    unsafe fn new(template: HDC, scale: f64) -> Option<Self> {
        let width = ((WHEEL_INNER * 2.0 - 12.0) * scale).round().max(1.0) as i32;
        let line_height = (16.0 * scale).round().max(1.0) as i32;
        let height = line_height.checked_mul(2)?;
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
        let font = CreateFontW(
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
            w!("Segoe UI"),
        );
        Some(Self {
            scale,
            font,
            dc,
            bitmap,
            old_bitmap,
            pixels: raw_pixels.cast(),
            width,
            height,
            line_height,
            usable: true,
            caption: None,
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
            if !self.font.is_null() {
                DeleteObject(self.font as HGDIOBJ);
            }
        }
    }
}

/// Only valid once all drawn inputs have settled. No pointer coordinates: they are not painted.
struct WheelDrawKey {
    kind: WheelKind,
    origin: Point,
    scale: f64,
    hot: Option<usize>,
    reduced: bool,
    theme: crate::appearance::Theme,
    pings: [PingId; 8],
    emotes: [String; 8],
}
impl WheelDrawKey {
    fn new(wheel: &WheelFx, settings: &Settings, reduced: bool) -> Self {
        Self {
            kind: wheel.kind,
            origin: wheel.origin,
            scale: wheel.scale,
            hot: wheel.hot,
            reduced,
            theme: crate::appearance::Theme::current(),
            pings: settings.wheel,
            emotes: settings.emote_wheel.clone(),
        }
    }
    fn matches(&self, wheel: &WheelFx, settings: &Settings, reduced: bool) -> bool {
        self.kind == wheel.kind
            && self.origin == wheel.origin
            && self.scale == wheel.scale
            && self.hot == wheel.hot
            && self.reduced == reduced
            && self.theme == crate::appearance::Theme::current()
            && match wheel.kind {
                WheelKind::Ping => self.pings == settings.wheel,
                WheelKind::Emote => self.emotes == settings.emote_wheel,
            }
    }
}

/// Exclusive screen-space bounds, including all animation travel and antialiasing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DrawBox {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
impl DrawBox {
    fn around(point: Point, radius: i32) -> Self {
        Self {
            left: point.x.saturating_sub(radius),
            top: point.y.saturating_sub(radius),
            right: point.x.saturating_add(radius).saturating_add(1),
            bottom: point.y.saturating_add(radius).saturating_add(1),
        }
    }
    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
    fn intersect(self, other: Self) -> Option<Self> {
        let r = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (r.left < r.right && r.top < r.bottom).then_some(r)
    }
    fn width(self) -> i32 {
        self.right - self.left
    }
    fn height(self) -> i32 {
        self.bottom - self.top
    }
    fn local(self, surface: Self) -> RECT {
        RECT {
            left: self.left - surface.left,
            top: self.top - surface.top,
            right: self.right - surface.left,
            bottom: self.bottom - surface.top,
        }
    }
    fn desktop() -> Option<Self> {
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 100_000_000 {
                return None;
            }
            Some(Self {
                left,
                top,
                right: left.checked_add(width)?,
                bottom: top.checked_add(height)?,
            })
        }
    }
}

fn effect_box(
    wheel: Option<&WheelFx>,
    pings: &[PingFx],
    emote: Option<&EmoteFx>,
    settings: &Settings,
    desktop: DrawBox,
) -> Option<DrawBox> {
    // ponytail: legacy width-based sampling can make custom/portrait art arbitrarily tall.
    // Keep full-desktop clipping there until it has aspect-aware geometry.
    if wheel.is_some_and(|w| {
        w.kind == WheelKind::Emote
            && settings
                .emote_wheel
                .iter()
                .any(|slug| pingloom_core::settings::is_custom_emote(slug))
    }) || emote.is_some_and(|e| {
        e.still.width() != e.still.height()
            || e.frames
                .as_ref()
                .is_some_and(|frames| frames.iter().any(|f| f.image.width() != f.image.height()))
    }) {
        return Some(desktop);
    }
    let wheel = wheel
        .map(|w| DrawBox::around(w.origin, (crate::wheel::EXTENT * w.scale).ceil() as i32 + 1));
    // Enclose ring expansion, two-layer convergence, bob and the still-pop overshoot.
    let pings = pings
        .iter()
        .map(|p| DrawBox::around(p.point, i32::from(settings.ping_size_px) * 2));
    let emote = emote.map(|e| DrawBox::around(e.point, i32::from(e.size)));
    wheel
        .into_iter()
        .chain(pings)
        .chain(emote)
        .reduce(DrawBox::union)
}

fn clear_box(pixels: &mut [u8], width: usize, rect: RECT) {
    let start = rect.left as usize * 4;
    let end = rect.right as usize * 4;
    for y in rect.top as usize..rect.bottom as usize {
        pixels[y * width * 4 + start..y * width * 4 + end].fill(0);
    }
    #[cfg(test)]
    crate::render_metrics::clear((end - start) * (rect.bottom - rect.top) as usize);
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
    desktop: DrawBox,
    drawn: Option<DrawBox>,
    last_wheel: Option<WheelDrawKey>,
    caption_cache: Vec<CaptionMask>,
    image_cache: HashMap<String, Arc<RgbaImage>>,
    reduced_motion: bool,
    motion_checked: Instant,
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
            let desktop = DrawBox::desktop().ok_or("invalid virtual desktop dimensions")?;
            // No desktop-sized allocation or visible overlay while idle.
            let (left, top, width, height) = (desktop.left, desktop.top, 1, 1);
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
            if old_bitmap.is_null() || old_bitmap as isize == -1 {
                DeleteObject(bitmap as HGDIOBJ);
                DeleteDC(dc);
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                return Err("could not select overlay surface".into());
            }
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
                desktop,
                drawn: None,
                last_wheel: None,
                caption_cache: Vec::new(),
                image_cache: HashMap::new(),
                reduced_motion: native_reduced_motion(),
                motion_checked: Instant::now(),
                pings: Vec::new(),
                emote: None,
                wheel: None,
            };
            Ok(result)
        }
    }

    fn surface_box(&self) -> DrawBox {
        DrawBox {
            left: self.left,
            top: self.top,
            right: self.left + self.width,
            bottom: self.top + self.height,
        }
    }

    unsafe fn resize_surface(&mut self, width: i32, height: i32) -> bool {
        if width == self.width && height == self.height {
            return true;
        }
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = null_mut();
        let bitmap = CreateDIBSection(self.dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
        if bitmap.is_null() || pixels.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap as HGDIOBJ);
            }
            return false;
        }
        GdiFlush();
        let previous = SelectObject(self.dc, bitmap as HGDIOBJ);
        if previous.is_null() || previous as isize == -1 {
            DeleteObject(bitmap as HGDIOBJ);
            return false;
        }
        DeleteObject(self.bitmap as HGDIOBJ);
        self.bitmap = bitmap;
        self.pixels = pixels.cast();
        self.width = width;
        self.height = height;
        if width == 1 && height == 1 {
            std::ptr::write_bytes(self.pixels, 0, 4);
        }
        self.drawn = None;
        true
    }

    /// Called after the owner's display-change debounce. Keep the HWND/owner/effects intact.
    pub fn refresh_display(&mut self) {
        let Some(desktop) = DrawBox::desktop() else {
            return;
        };
        if desktop == self.desktop {
            return;
        }
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            // Release the old desktop's storage before any next-frame growth.
            self.resize_surface(1, 1);
        }
        self.caption_cache.clear();
        if let Some(wheel) = &mut self.wheel {
            wheel.caption_mask = None;
            wheel.caption_mask = unsafe { CaptionMask::new(self.dc, wheel.scale) };
        }
        self.desktop = desktop;
        self.left = desktop.left;
        self.top = desktop.top;
        self.drawn = None;
        self.last_wheel = None;
    }

    unsafe fn prepare_frame(&mut self, drawn: DrawBox) -> Option<RECT> {
        let old_surface = self.surface_box();
        let area = i64::from(drawn.width()) * i64::from(drawn.height());
        let resize = drawn.width() > self.width
            || drawn.height() > self.height
            || area < i64::from(self.width) * i64::from(self.height) / 4
            || self.width > self.desktop.width()
            || self.height > self.desktop.height();
        if resize && !self.resize_surface(drawn.width(), drawn.height()) {
            return None;
        }
        self.left = drawn
            .left
            .min(self.desktop.right - self.width)
            .max(self.desktop.left);
        self.top = drawn
            .top
            .min(self.desktop.bottom - self.height)
            .max(self.desktop.top);
        let surface = self.surface_box();
        let dirty = match self.drawn {
            Some(previous) if surface == old_surface => previous.union(drawn).intersect(surface)?,
            _ => surface,
        };
        let rect = dirty.local(surface);
        clear_box(
            std::slice::from_raw_parts_mut(
                self.pixels,
                self.width as usize * self.height as usize * 4,
            ),
            self.width as usize,
            rect,
        );
        Some(rect)
    }

    unsafe fn present(&self, dirty: &RECT) -> bool {
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
        let info = UPDATELAYEREDWINDOWINFO {
            cbSize: size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
            hdcDst: null_mut(),
            pptDst: &destination,
            psize: &size,
            hdcSrc: self.dc,
            pptSrc: &source,
            crKey: 0,
            pblend: &blend,
            dwFlags: ULW_ALPHA,
            prcDirty: dirty,
        };
        let indirect = UpdateLayeredWindowIndirect(self.hwnd, &info) != 0;
        #[cfg(test)]
        assert!(
            indirect,
            "dirty-rect fast path must work in native tests, not silently fall back: {}",
            std::io::Error::last_os_error()
        );
        indirect
            || UpdateLayeredWindow(
                self.hwnd,
                null_mut(),
                &destination,
                &size,
                self.dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            ) != 0
    }

    pub fn wheel_open(&mut self, kind: WheelKind, origin: Point, scale: f64) {
        let _trace = crate::trace::probe(crate::trace::Metric::WheelOpen);
        self.wheel_close();
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let caption_mask = if let Some(index) = self
            .caption_cache
            .iter()
            .position(|mask| mask.scale == scale)
        {
            Some(self.caption_cache.swap_remove(index))
        } else {
            unsafe { CaptionMask::new(self.dc, scale) }
        };
        self.wheel = Some(WheelFx {
            kind,
            origin,
            pointer: origin,
            scale,
            hot: None,
            selection: [0.0; 8],
            opened: Instant::now(),
            updated: Instant::now(),
            caption_mask,
            surface: crate::wheel::Surface::new(scale),
            layer: None,
            scaled_art: Vec::new(),
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
        if let Some(mut wheel) = self.wheel.take() {
            if let Some(mask) = wheel.caption_mask.take().filter(|mask| mask.usable) {
                if self.caption_cache.len() == 8 {
                    self.caption_cache.remove(0);
                }
                self.caption_cache.push(mask);
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
        let path = format!("emotes/{slug}.png");
        let bytes = crate::custom_emotes::asset(&path)
            .map(std::borrow::Cow::Owned)
            .or_else(|| assets::get(&path))?;
        let still = image::load_from_memory(&bytes).ok()?.into_rgba8();
        let pending = !self.reduced_motion && crate::animation::has_animation(slug);
        self.emote = Some(EmoteFx {
            slug: slug.to_owned(),
            point,
            size,
            started: Instant::now(),
            still,
            frames: None,
            animation_started: None,
            animation_pending: pending,
        });
        pending.then(|| slug.to_owned())
    }

    pub fn set_emote_frames(&mut self, slug: &str, frames: Vec<AnimatedFrame>) {
        if let Some(emote) = &mut self.emote {
            if emote.slug == slug && emote.frames.is_none() {
                emote.complete_decode(frames, Instant::now());
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.wheel.is_some() || !self.pings.is_empty() || self.emote.is_some()
    }

    pub fn tick(&mut self, settings: &Settings) {
        let _trace = crate::trace::probe(crate::trace::Metric::Render);
        let now = Instant::now();
        // Refresh the OS preference without querying it for every rendered frame.
        if now.duration_since(self.motion_checked) >= Duration::from_secs(1) {
            self.reduced_motion = native_reduced_motion();
            self.motion_checked = now;
        }
        let duration = Duration::from_secs_f32(settings.ping_duration_s);
        self.pings
            .retain(|ping| now.duration_since(ping.started) < duration);
        if self.emote.as_ref().is_some_and(|emote| {
            !emote.animation_pending && now.duration_since(emote.started) >= EMOTE_LIFETIME
        }) {
            self.emote = None;
        }
        if let Some(wheel) = self.wheel.as_mut() {
            wheel.advance(now, self.reduced_motion);
        }
        let settled = self.pings.is_empty()
            && self.emote.is_none()
            && self
                .wheel
                .as_ref()
                .is_some_and(|wheel| wheel.settled(now, self.reduced_motion));
        if settled
            && self.drawn.is_some()
            && unsafe { IsWindowVisible(self.hwnd) } != 0
            && self
                .last_wheel
                .as_ref()
                .zip(self.wheel.as_ref())
                .is_some_and(|(key, wheel)| key.matches(wheel, settings, self.reduced_motion))
        {
            return;
        }
        self.last_wheel = None;
        let drawn = effect_box(
            self.wheel.as_ref(),
            &self.pings,
            self.emote.as_ref(),
            settings,
            self.desktop,
        )
        .and_then(|r| r.intersect(self.desktop));
        let Some(drawn) = drawn else {
            unsafe {
                if IsWindowVisible(self.hwnd) != 0 {
                    ShowWindow(self.hwnd, SW_HIDE);
                }
                if let Some(previous) = self
                    .drawn
                    .take()
                    .and_then(|r| r.intersect(self.surface_box()))
                {
                    let rect = previous.local(self.surface_box());
                    clear_box(
                        std::slice::from_raw_parts_mut(
                            self.pixels,
                            self.width as usize * self.height as usize * 4,
                        ),
                        self.width as usize,
                        rect,
                    );
                }
                self.resize_surface(1, 1);
            }
            return;
        };

        unsafe {
            let Some(dirty) = self.prepare_frame(drawn) else {
                crate::trace::event(crate::trace::Event::OverlayFailure);
                ShowWindow(self.hwnd, SW_HIDE);
                self.drawn = None;
                return;
            };
            let was_hidden = IsWindowVisible(self.hwnd) == 0;
            let len = (self.width as usize) * (self.height as usize) * 4;
            let mut canvas = Canvas {
                pixels: std::slice::from_raw_parts_mut(self.pixels, len),
                width: self.width as usize,
                height: self.height as usize,
                left: self.left,
                top: self.top,
                opacity: 1.0,
            };
            if let Some(wheel) = &mut self.wheel {
                if !self.reduced_motion {
                    let progress = (now.saturating_duration_since(wheel.opened).as_secs_f64()
                        / 0.12)
                        .clamp(0.0, 1.0);
                    canvas.opacity = 0.75 + 0.25 * (1.0 - (1.0 - progress).powi(3));
                }
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
                wheel.surface.refresh(wheel.scale);
                canvas.draw_wheel(wheel, &items, &mut self.image_cache);
                let selected = wheel
                    .hot
                    .and_then(|slot| names.get(slot))
                    .map(String::as_str);
                draw_wheel_caption(&mut canvas, wheel, selected);
                canvas.opacity = 1.0;
            }
            for ping in &self.pings {
                let age = now.duration_since(ping.started).as_secs_f64();
                canvas.draw_ping(
                    ping,
                    settings,
                    age,
                    &mut self.image_cache,
                    self.reduced_motion,
                );
            }
            if let Some(emote) = &self.emote {
                let image = emote.image_at(now, self.reduced_motion);
                let life = now.duration_since(emote.started).as_secs_f64();
                canvas.draw_emote(emote, image, life, self.reduced_motion);
            }
            let presented = self.present(&dirty);
            if presented && was_hidden {
                // Upload before showing: never flash the previous surface at its old position.
                // Re-raise above other topmost windows, without taking activation.
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            } else if !presented {
                crate::trace::event(crate::trace::Event::OverlayFailure);
                ShowWindow(self.hwnd, SW_HIDE);
            }
            // A failed present must retry with a complete frame, not a stale dirty region.
            self.drawn = presented.then_some(drawn);
            if presented && settled {
                self.last_wheel = self
                    .wheel
                    .as_ref()
                    .map(|wheel| WheelDrawKey::new(wheel, settings, self.reduced_motion));
            }
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
    // The atlas owns the V7 hub/direction text. Keep the dynamic action name below it.
    let Some(selected) = selected else {
        return;
    };
    let Some(mask) = wheel.caption_mask.as_mut() else {
        return;
    };
    if !mask.usable {
        return;
    }
    if mask.caption.as_deref() != Some(selected) {
        let bytes = mask.width as usize * mask.height as usize * 4;
        std::ptr::write_bytes(mask.pixels, 0, bytes);
        let font = if mask.font.is_null() {
            GetStockObject(DEFAULT_GUI_FONT)
        } else {
            mask.font as HGDIOBJ
        };
        let old_font = SelectObject(mask.dc, font);
        let old_mode = SetBkMode(mask.dc, TRANSPARENT as i32);
        let old_color = SetTextColor(mask.dc, 0x00f8_f8f8);
        let (title, instruction) = (selected.to_owned(), "Release to choose".to_owned());
        let title = title
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let instruction = instruction
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        for (text, line) in [(title.as_ptr(), 0), (instruction.as_ptr(), 1)] {
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
        mask.caption = Some(selected.to_owned());
    }

    let left = wheel.origin.x - mask.width / 2;
    let top = wheel.origin.y + ((WHEEL_OUTER + 16.0) * wheel.scale).round() as i32;
    for y in 0..mask.height {
        for x in 0..mask.width {
            let index = (y as usize * mask.width as usize + x as usize) * 4;
            let coverage = (*mask.pixels.add(index))
                .max(*mask.pixels.add(index + 1))
                .max(*mask.pixels.add(index + 2));
            if coverage != 0 {
                let ink = wheel.surface.theme.palette().ink;
                canvas.point(left + x, top + y, [ink[0], ink[1], ink[2], coverage]);
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

struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: usize,
    height: usize,
    left: i32,
    top: i32,
    opacity: f64,
}

impl Canvas<'_> {
    fn point(&mut self, x: i32, y: i32, mut rgba: [u8; 4]) {
        let x = x - self.left;
        let y = y - self.top;
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        #[cfg(test)]
        crate::render_metrics::write(4);
        let index = (y as usize * self.width + x as usize) * 4;
        let destination = [
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
            self.pixels[index + 3],
        ];
        if self.opacity != 1.0 {
            rgba[3] = (f64::from(rgba[3]) * self.opacity) as u8;
        }
        let blended = composite_premultiplied_bgra(destination, rgba);
        self.pixels[index..index + 4].copy_from_slice(&blended);
    }

    fn circle(&mut self, center: Point, radius: f64, thickness: f64, color: [u8; 4]) {
        let outer = radius + thickness / 2.0 + 0.5;
        let inner = (radius - thickness / 2.0 - 0.5).max(0.0);
        for dy in -(outer.ceil() as i32)..=outer.ceil() as i32 {
            let y2 = f64::from(dy).powi(2);
            if y2 > outer * outer {
                continue;
            }
            let end = (outer * outer - y2).sqrt().floor() as i32;
            let start = (inner * inner - y2).max(0.0).sqrt().ceil() as i32;
            for dx in start..=end {
                let distance = f64::from(dx).hypot(f64::from(dy));
                if distance < inner || distance > outer {
                    continue;
                }
                let mut rgba = color;
                rgba[3] = (f64::from(color[3]) * ring_coverage(distance, radius, thickness)) as u8;
                self.point(center.x + dx, center.y + dy, rgba);
                if dx != 0 {
                    self.point(center.x - dx, center.y + dy, rgba);
                }
            }
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
        let image = crate::raster::resize_art(image, size);
        let size = image.width() as usize;
        let height = image.height() as usize;
        for dy in 0..height {
            for dx in 0..size {
                let pixel = image.get_pixel(dx as u32, dy as u32).0;
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
        let bytes = crate::custom_emotes::asset(path)
            .map(std::borrow::Cow::Owned)
            .or_else(|| assets::get(path))?;
        let image = Arc::new(image::load_from_memory(&bytes).ok()?.into_rgba8());
        cache.insert(path.to_owned(), Arc::clone(&image));
        Some(image)
    }

    // Wheel is the first effect on the cleared canvas. Direct replacement is
    // intentional: blending a selected pixel over the idle pixel changes alpha.
    fn draw_wheel_surface(&mut self, wheel: &mut WheelFx) {
        if self.opacity != 1.0 {
            wheel.surface.paint(&wheel.selection, |dx, dy, rgba| {
                self.point(wheel.origin.x + dx, wheel.origin.y + dy, rgba);
            });
            return;
        }
        if wheel
            .layer
            .as_ref()
            .is_none_or(|layer| layer.theme != wheel.surface.theme)
        {
            wheel.layer = Some(WheelLayer::new(&wheel.surface, wheel.scale));
        }
        let layer = wheel.layer.as_ref().unwrap();
        let width = (layer.extent * 2 + 1) as usize;
        let left = wheel.origin.x - layer.extent - self.left;
        let top = wheel.origin.y - layer.extent - self.top;
        let x0 = left.max(0) as usize;
        let x1 = (left + width as i32).clamp(0, self.width as i32) as usize;
        let y0 = top.max(0) as usize;
        let y1 = (top + width as i32).clamp(0, self.height as i32) as usize;
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let source = ((y as i32 - top) as usize * width + (x0 as i32 - left) as usize) * 4;
            let destination = (y * self.width + x0) * 4;
            let bytes = (x1 - x0) * 4;
            self.pixels[destination..destination + bytes]
                .copy_from_slice(&layer.pixels[source..source + bytes]);
            #[cfg(test)]
            crate::render_metrics::write(bytes);
        }
        wheel
            .surface
            .paint_selection(&wheel.selection, |dx, dy, rgba| {
                let x = wheel.origin.x + dx - self.left;
                let y = wheel.origin.y + dy - self.top;
                if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                    let index = (y as usize * self.width + x as usize) * 4;
                    let mut pixel = composite_premultiplied_bgra([0; 4], rgba);
                    if let Some(labels) = &wheel.surface.labels {
                        let lx = dx + labels.width() as i32 / 2;
                        let ly = dy + labels.width() as i32 / 2;
                        if lx >= 0
                            && ly >= 0
                            && (lx as u32) < labels.width()
                            && (ly as u32) < labels.height()
                        {
                            pixel = composite_premultiplied_bgra(
                                pixel,
                                labels.get_pixel(lx as u32, ly as u32).0,
                            );
                        }
                    }
                    self.pixels[index..index + 4].copy_from_slice(&pixel);
                    #[cfg(test)]
                    crate::render_metrics::write(4);
                }
            });
    }

    fn draw_wheel(
        &mut self,
        wheel: &mut WheelFx,
        items: &[String],
        cache: &mut HashMap<String, Arc<RgbaImage>>,
    ) {
        let scale = wheel.scale;
        self.draw_wheel_surface(wheel);
        // Entrance fade keeps the original per-primitive alpha rounding. Once
        // opaque, the labels are already baked into the layer and its patches.
        if let Some(labels) = wheel
            .surface
            .labels
            .as_ref()
            .filter(|_| self.opacity != 1.0)
        {
            let size = (crate::wheel::EXTENT * 2.0 * scale).round() as u32;
            self.image(
                labels,
                wheel.origin.x - size as i32 / 2,
                wheel.origin.y - size as i32 / 2,
                size,
                1.0,
            );
        }
        for (slot, item) in items.iter().enumerate() {
            let angle = (slot as f64 * 45.0 - 90.0).to_radians();
            let radius = crate::wheel::ICON_RADIUS * scale;
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
                let icon_size = (46.0 * scale * (1.0 + 0.18 * wheel.selection[slot])) as u32;
                let icon = wheel.icon(&image_name, &icon, icon_size);
                self.image(
                    &icon,
                    center.x - icon_size as i32 / 2,
                    center.y - icon_size as i32 / 2,
                    icon_size,
                    0.72 + 0.28 * wheel.selection[slot],
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
        reduced_motion: bool,
    ) {
        let asset = ping.id.asset();
        let size = f64::from(settings.ping_size_px);
        let ping_color = color(asset.color);
        let fade = effect_fade(age, f64::from(settings.ping_duration_s));
        self.ellipse(
            ping.point,
            size * 0.42,
            size * 0.12,
            [
                ping_color[0],
                ping_color[1],
                ping_color[2],
                (85.0 * fade) as u8,
            ],
        );
        for (delay, ring_alpha) in [(0.0, 230), (0.22, 210), (1.4, 190)] {
            let t = age - delay;
            if !reduced_motion && (0.0..0.65).contains(&t) {
                let radius = size * 0.545 * (0.25 + 1.10 * t / 0.65);
                let alpha = (f64::from(ring_alpha) * (1.0 - t / 0.65) * fade) as u8;
                self.circle(
                    ping.point,
                    radius,
                    3.0,
                    [ping_color[0], ping_color[1], ping_color[2], alpha],
                );
            }
        }
        for (index, art) in asset.art.iter().enumerate() {
            let Some(image) = self.asset(cache, &format!("textures/{art}.png")) else {
                continue;
            };
            let (x, y, scale, alpha) = ping_art_motion(age, index, asset.art.len(), reduced_motion);
            let art_size = (size * scale) as u32;
            self.image(
                &image,
                ping.point.x - art_size as i32 / 2 + (size * x) as i32,
                ping.point.y - art_size as i32 / 2 - (size * 0.23) as i32 + (size * y) as i32,
                art_size,
                fade * alpha,
            );
        }
    }

    fn draw_emote(&mut self, emote: &EmoteFx, image: &RgbaImage, age: f64, reduced_motion: bool) {
        let size = f64::from(emote.size);
        let baked = emote
            .frames
            .as_ref()
            .is_some_and(|frames| !frames.is_empty());
        let (pop, fade) = emote_envelope(age, baked, reduced_motion, emote.animation_pending);
        if fade <= 0.0 {
            return;
        }
        let draw_size = (size * 1.25 * pop) as u32;
        self.image(
            image,
            emote.point.x - draw_size as i32 / 2,
            emote.point.y - draw_size as i32 / 2,
            draw_size,
            fade,
        );
    }
}

fn native_reduced_motion() -> bool {
    let mut enabled = 1i32;
    let available = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        )
    };
    available != 0 && enabled == 0
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

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_evidence;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v7_native_canvas_presents_the_shared_blush_wheel() {
        let mut pixels = vec![0; 500 * 500 * 4];
        let mut canvas = Canvas {
            pixels: &mut pixels,
            width: 500,
            height: 500,
            left: 0,
            top: 0,
            opacity: 1.0,
        };
        let now = Instant::now();
        let mut selection = [0.0; 8];
        selection[0] = 1.0;
        let mut wheel = WheelFx {
            kind: WheelKind::Ping,
            origin: Point::new(250, 250),
            pointer: Point::new(250, 50),
            scale: 1.0,
            hot: Some(0),
            selection,
            opened: now,
            updated: now,
            caption_mask: None,
            surface: crate::wheel::Surface::new(1.0),
            layer: None,
            scaled_art: Vec::new(),
        };
        assert!(wheel.surface.labels.is_some());
        canvas.draw_wheel(&mut wheel, &[], &mut HashMap::new());
        let pixel = |x: usize, y: usize| &canvas.pixels[(y * 500 + x) * 4..(y * 500 + x) * 4 + 4];
        assert_eq!(pixel(250, 156)[3], 0, "hub/wedge gap must stay transparent");
        assert_eq!(
            pixel(250, 34),
            composite_premultiplied_bgra([0; 4], [226, 180, 189, 250])
        );
        assert_eq!(
            pixel(280, 290),
            composite_premultiplied_bgra([0; 4], [26, 23, 27, 250])
        );
    }

    #[test]
    fn completion_never_flashes_thumbnail_and_starts_a_fresh_lifetime() {
        let start = Instant::now();
        let pixel = |value| RgbaImage::from_pixel(1, 1, image::Rgba([value, 0, 0, 255]));
        let mut emote = EmoteFx {
            slug: "facepalm".into(),
            point: Point::new(0, 0),
            size: 150,
            started: start,
            still: pixel(9),
            frames: None,
            animation_started: None,
            animation_pending: true,
        };
        assert_eq!(
            emote_envelope(0.5, false, false, emote.animation_pending).1,
            0.0
        );
        let ready = start + Duration::from_millis(500);
        emote.complete_decode(
            vec![AnimatedFrame {
                at_ms: 0,
                end_ms: 100,
                image: Arc::new(pixel(1)),
            }],
            ready,
        );
        assert!(!emote.animation_pending);
        assert_eq!(emote.started, ready);
        assert_eq!(emote.animation_started, Some(ready));
        assert_eq!(emote.image_at(ready, false).get_pixel(0, 0)[0], 1);
        assert_eq!(
            emote_envelope(0.0, true, false, emote.animation_pending),
            (1.0, 1.0)
        );
        // Failed or static-only decoding must also leave pending and show the still safely.
        emote.complete_decode(Vec::new(), ready + Duration::from_millis(100));
        assert!(!emote.animation_pending);
        assert_eq!(
            emote
                .image_at(ready + Duration::from_millis(100), false)
                .get_pixel(0, 0)[0],
            9
        );
        assert!(emote_envelope(0.1, false, false, emote.animation_pending).1 > 0.0);
        assert_eq!(emote.image_at(ready, true).get_pixel(0, 0)[0], 9);
    }

    #[test]
    fn playback_uses_decode_ready_clock_and_stills_for_reduced_motion() {
        let now = Instant::now();
        let pixel = |value| RgbaImage::from_pixel(1, 1, image::Rgba([value, 0, 0, 255]));
        let emote = EmoteFx {
            slug: "test".into(),
            point: Point::new(0, 0),
            size: 80,
            started: now - Duration::from_secs(2),
            still: pixel(9),
            frames: Some(vec![
                AnimatedFrame {
                    at_ms: 0,
                    end_ms: 100,
                    image: Arc::new(pixel(1)),
                },
                AnimatedFrame {
                    at_ms: 100,
                    end_ms: 200,
                    image: Arc::new(pixel(2)),
                },
            ]),
            animation_started: Some(now),
            animation_pending: false,
        };
        assert_eq!(emote.image_at(now, false).get_pixel(0, 0).0[0], 1);
        assert_eq!(
            emote
                .image_at(now + Duration::from_millis(99), false)
                .get_pixel(0, 0)
                .0[0],
            1
        );
        assert_eq!(
            emote
                .image_at(now + Duration::from_millis(100), false)
                .get_pixel(0, 0)
                .0[0],
            2
        );
        assert_eq!(
            emote
                .image_at(now + Duration::from_secs(1), false)
                .get_pixel(0, 0)
                .0[0],
            2
        );
        assert_eq!(emote.image_at(now, true).get_pixel(0, 0).0[0], 9);
        let custom = EmoteFx {
            slug: "custom_0123456789abcdef".into(),
            ..emote
        };
        assert_eq!(
            custom
                .image_at(now + Duration::from_millis(150), false)
                .get_pixel(0, 0)[0],
            2
        );
        assert_eq!(
            custom
                .image_at(now + Duration::from_millis(210), false)
                .get_pixel(0, 0)[0],
            1
        );
        assert_eq!(
            custom
                .image_at(now + Duration::from_millis(150), true)
                .get_pixel(0, 0)[0],
            9
        );
    }

    #[test]
    fn decoded_webp_frames_begin_at_zero_and_use_previous_frame_end() {
        let frames = decode_emote("facepalm").expect("bundled animated emote must decode");
        let bytes = assets::get("emotes/facepalm.webp").unwrap();
        let mut previous_end = 0;
        for (frame, decoded) in frames.iter().zip(Decoder::new(&bytes).unwrap()) {
            assert_eq!(frame.at_ms, previous_end);
            previous_end = decoded.timestamp().max(0) as u32;
        }
        assert!(frames.len() > 1);
        assert_eq!(frames[0].at_ms, 0);
        assert_eq!(frames[1].at_ms, 33);
    }
}
