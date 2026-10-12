use std::collections::HashMap;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use lolping_core::model::{PingId, WheelKind};
use lolping_core::settings::Settings;
use lolping_core::{geometry::pick_slice, Point};
use webp_animation::Decoder;
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, SIZE};
use windows_sys::Win32::Graphics::Gdi::BLENDFUNCTION;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
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

struct WheelFx {
    kind: WheelKind,
    origin: Point,
    pointer: Point,
    scale: f64,
    hot: Option<usize>,
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
        lpszClassName: w!("lolping-overlay"),
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
                w!("lolping-overlay"),
                w!("lolPing overlay"),
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
        self.wheel = Some(WheelFx {
            kind,
            origin,
            pointer: origin,
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
            hot: None,
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
        self.wheel = None;
    }

    pub fn wheel_close_if(&mut self, kind: WheelKind, origin: Point) {
        if self
            .wheel
            .as_ref()
            .is_some_and(|wheel| wheel.kind == kind && wheel.origin == origin)
        {
            self.wheel = None;
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
            if let Some(wheel) = &self.wheel {
                let items: Vec<String> = match wheel.kind {
                    WheelKind::Ping => settings
                        .wheel
                        .iter()
                        .map(|id| id.asset().icon.to_owned())
                        .collect(),
                    WheelKind::Emote => settings.emote_wheel.to_vec(),
                };
                let center = match wheel.kind {
                    WheelKind::Ping => settings.click_ping_id.asset().icon.to_owned(),
                    WheelKind::Emote => settings.click_emote_id.clone(),
                };
                canvas.draw_wheel(wheel, &items, &center, &mut self.image_cache);
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
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
            SelectObject(self.dc, self.old_bitmap);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
        }
    }
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
        let alpha = u32::from(rgba[3]);
        let inverse = 255 - alpha;
        self.pixels[index] =
            ((u32::from(rgba[2]) * alpha + u32::from(self.pixels[index]) * inverse) / 255) as u8;
        self.pixels[index + 1] = ((u32::from(rgba[1]) * alpha
            + u32::from(self.pixels[index + 1]) * inverse)
            / 255) as u8;
        self.pixels[index + 2] = ((u32::from(rgba[0]) * alpha
            + u32::from(self.pixels[index + 2]) * inverse)
            / 255) as u8;
        self.pixels[index + 3] =
            (alpha + u32::from(self.pixels[index + 3]) * inverse / 255).min(255) as u8;
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
        center: &str,
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
                        [170, 164, 148, 175]
                    } else {
                        [43, 49, 55, 200]
                    };
                    self.point(wheel.origin.x + dx, wheel.origin.y + dy, color);
                }
            }
        }
        self.circle(wheel.origin, inner, 3.0 * scale, [200, 170, 110, 230]);
        self.circle(wheel.origin, outer, 2.0 * scale, [150, 130, 90, 180]);
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
                self.image(
                    &icon,
                    center.x - icon_size as i32 / 2,
                    center.y - icon_size as i32 / 2,
                    icon_size,
                    1.0,
                );
            }
        }
        let path = if wheel.kind == WheelKind::Ping {
            format!("textures/{center}.png")
        } else {
            format!("emotes/{center}.png")
        };
        if let Some(icon) = self.asset(cache, &path) {
            let size = (56.0 * scale) as u32;
            self.image(
                &icon,
                wheel.origin.x - size as i32 / 2,
                wheel.origin.y - size as i32 / 2,
                size,
                1.0,
            );
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
