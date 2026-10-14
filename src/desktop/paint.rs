//! A native Tauri Window, never a WebView. Keep at most one pending GUI frame.
use super::render::Frame;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::AppHandle;
#[cfg(target_os = "linux")]
use tauri::{PhysicalPosition, PhysicalSize};

pub(super) struct Painter {
    app: AppHandle,
    latest: Arc<Mutex<Option<Arc<Frame>>>>,
    displayed: Arc<Mutex<Option<Arc<Frame>>>>,
    queued: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    reduced: Arc<AtomicBool>,
}
impl Painter {
    pub fn new(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let window = tauri::WindowBuilder::new(app, "native-overlay")
            .title("Pingloom overlay")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .visible(false)
            .inner_size(1.0, 1.0)
            .build()?;
        window.set_ignore_cursor_events(true)?;
        let latest = Arc::new(Mutex::new(None::<Arc<Frame>>));
        let displayed = Arc::new(Mutex::new(None::<Arc<Frame>>));
        install(&window, Arc::clone(&displayed))?;
        Ok(Self {
            app: app.clone(),
            latest,
            displayed,
            queued: Arc::new(AtomicBool::new(false)),
            failed: Arc::new(AtomicBool::new(false)),
            visible: Arc::new(AtomicBool::new(false)),
            reduced: Arc::new(AtomicBool::new(system_reduced_motion())),
        })
    }
    pub fn reduced_motion(&self) -> bool {
        self.reduced.load(Ordering::Acquire)
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    pub fn present(&self, frame: Option<Frame>) {
        {
            let mut latest = self.latest.lock().unwrap();
            if frame.is_none()
                && latest.is_none()
                && !self.visible.load(Ordering::Acquire)
                && !self.queued.load(Ordering::Acquire)
            {
                return;
            }
            *latest = frame.map(Arc::new);
        }
        if self.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let app = self.app.clone();
        let latest = Arc::clone(&self.latest);
        let displayed = Arc::clone(&self.displayed);
        let queued = Arc::clone(&self.queued);
        let failed = Arc::clone(&self.failed);
        let visible = Arc::clone(&self.visible);
        let reduced = Arc::clone(&self.reduced);
        if self
            .app
            .run_on_main_thread(move || {
                use tauri::Manager;
                reduced.store(system_reduced_motion(), Ordering::Release);
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let window = app
                        .get_window("native-overlay")
                        .ok_or("native overlay unavailable")?;
                    let frame = latest.lock().unwrap().clone();
                    *displayed.lock().unwrap() = frame.clone();
                    if let Some(frame) = frame {
                        #[cfg(target_os = "linux")]
                        {
                            window.set_position(PhysicalPosition::new(frame.x, frame.y))?;
                            window.set_size(PhysicalSize::new(
                                frame.image.width(),
                                frame.image.height(),
                            ))?;
                        }
                        #[cfg(target_os = "macos")]
                        {
                            window.set_position(tauri::LogicalPosition::new(
                                frame.x as f64,
                                frame.y as f64,
                            ))?;
                            window.set_size(tauri::LogicalSize::new(
                                frame.image.width() as f64,
                                frame.image.height() as f64,
                            ))?;
                        }
                        update(&window, &frame)?;
                        window.show()?;
                        visible.store(true, Ordering::Release);
                    } else {
                        window.hide()?;
                        visible.store(false, Ordering::Release);
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    eprintln!("Native overlay failed: {error}");
                    failed.store(true, Ordering::Release);
                }
                queued.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.queued.store(false, Ordering::Release);
            self.failed.store(true, Ordering::Release);
        }
    }
}

#[cfg(target_os = "linux")]
fn system_reduced_motion() -> bool {
    use gtk::prelude::*;
    gtk::Settings::default().is_some_and(|settings| !settings.is_gtk_enable_animations())
}
#[cfg(target_os = "macos")]
fn system_reduced_motion() -> bool {
    use objc2::{
        class, msg_send,
        runtime::{AnyObject, Bool},
    };
    unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduced: Bool = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduced.as_bool()
    }
}

#[cfg(target_os = "linux")]
fn install(
    window: &tauri::Window,
    latest: Arc<Mutex<Option<Arc<Frame>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use gtk::prelude::*;
    let area = gtk::DrawingArea::new();
    area.connect_draw(move |area, context| {
        let frame = latest.lock().unwrap().clone();
        if let Some(frame) = frame {
            let mut pixels = frame.image.as_raw().clone();
            for pixel in pixels.as_chunks_mut::<4>().0 {
                let alpha = pixel[3] as u16;
                let r = (pixel[0] as u16 * alpha / 255) as u8;
                let g = (pixel[1] as u16 * alpha / 255) as u8;
                let b = (pixel[2] as u16 * alpha / 255) as u8;
                pixel[0] = b;
                pixel[1] = g;
                pixel[2] = r;
            }
            if let Ok(surface) = gtk::cairo::ImageSurface::create_for_data(
                pixels,
                gtk::cairo::Format::ARgb32,
                frame.image.width() as i32,
                frame.image.height() as i32,
                frame.image.width() as i32 * 4,
            ) {
                let _ = context.save();
                let scale = area.scale_factor() as f64;
                context.scale(1.0 / scale, 1.0 / scale);
                context.set_operator(gtk::cairo::Operator::Source);
                if context.set_source_surface(&surface, 0.0, 0.0).is_ok() {
                    let _ = context.paint();
                }
                let _ = context.restore();
            }
        }
        gtk::glib::Propagation::Proceed
    });
    let container = window.default_vbox()?;
    container.pack_start(&area, true, true, 0);
    area.show();
    Ok(())
}
#[cfg(target_os = "linux")]
fn update(window: &tauri::Window, _frame: &Frame) -> Result<(), Box<dyn std::error::Error>> {
    use gtk::prelude::*;
    window.default_vbox()?.queue_draw();
    Ok(())
}

#[cfg(target_os = "macos")]
fn install(
    window: &tauri::Window,
    _latest: Arc<Mutex<Option<Arc<Frame>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use objc2::{class, msg_send, runtime::AnyObject};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    unsafe {
        let native = window.ns_window()? as *mut AnyObject;
        let view: *mut AnyObject = msg_send![class!(NSImageView), alloc];
        let view: *mut AnyObject =
            msg_send![view,initWithFrame:NSRect::new(NSPoint::new(0.0,0.0),NSSize::new(1.0,1.0))];
        if view.is_null() {
            return Err("could not create native image view".into());
        }
        let _: () = msg_send![view,setAutoresizingMask:18usize];
        let _: () = msg_send![view,setImageScaling:2usize];
        let _: () = msg_send![native,setContentView:view];
        let _: () = msg_send![native,setCollectionBehavior:257usize];
        let _: () = msg_send![view, release];
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn update(window: &tauri::Window, frame: &Frame) -> Result<(), Box<dyn std::error::Error>> {
    use image::ImageEncoder;
    use objc2::{class, msg_send, runtime::AnyObject};
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut png,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(
        frame.image.as_raw(),
        frame.image.width(),
        frame.image.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    let data = objc2_foundation::NSData::with_bytes(&png);
    unsafe {
        let native = window.ns_window()? as *mut AnyObject;
        let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
        let image: *mut AnyObject = msg_send![image,initWithData:&*data];
        if image.is_null() {
            return Err("could not decode native frame".into());
        }
        let view: *mut AnyObject = msg_send![native, contentView];
        let _: () = msg_send![view,setImage:image];
        let _: () = msg_send![image, release];
    }
    Ok(())
}
