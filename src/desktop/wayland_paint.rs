//! Native passive Wayland overlay using gtk-layer-shell.
//!
//! Provides a click-through, non-focus-stealing transparent overlay across all
//! connected Wayland monitors using the zwlr_layer_shell_v1 protocol.

use std::{cell::RefCell, error::Error, rc::Rc, sync::Arc};

use gtk::prelude::*;
use gtk_layer_shell::LayerShell;

use super::render::Frame;

thread_local! {
    static STATE: RefCell<Option<WaylandState>> = const { RefCell::new(None) };
}

struct WaylandState {
    display: gtk::gdk::Display,
    monitors: Vec<MonitorOverlay>,
}

struct MonitorOverlay {
    monitor: gtk::gdk::Monitor,
    window: gtk::Window,
    drawing_area: gtk::DrawingArea,
    current_crop: Rc<RefCell<Option<CropSurface>>>,
}

struct CropSurface {
    surface: gtk::cairo::ImageSurface,
    dest_x: i32,
    dest_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MonitorCrop {
    /// Frame-relative source X offset in logical pixels.
    pub src_x: u32,
    /// Frame-relative source Y offset in logical pixels.
    pub src_y: u32,
    /// Width of the cropped region in logical pixels.
    pub width: u32,
    /// Height of the cropped region in logical pixels.
    pub height: u32,
    /// Destination X on the monitor in logical pixels.
    pub dest_x: i32,
    /// Destination Y on the monitor in logical pixels.
    pub dest_y: i32,
}

/// Compute the intersection and cropping parameters between a frame and a monitor geometry.
///
/// Both `frame` and `geom` coordinates are in global logical output-layout space.
#[allow(clippy::too_many_arguments)] // Two rectangles, kept independent of GTK for the pure check.
pub(super) fn compute_crop(
    frame_x: i32,
    frame_y: i32,
    frame_w: u32,
    frame_h: u32,
    geom_x: i32,
    geom_y: i32,
    geom_w: i32,
    geom_h: i32,
) -> Option<MonitorCrop> {
    if frame_w == 0 || frame_h == 0 || geom_w <= 0 || geom_h <= 0 {
        return None;
    }

    let frame_w_i64 = frame_w as i64;
    let frame_h_i64 = frame_h as i64;
    let geom_w_i64 = geom_w as i64;
    let geom_h_i64 = geom_h as i64;

    let fx0 = frame_x as i64;
    let fy0 = frame_y as i64;
    let fx1 = fx0 + frame_w_i64;
    let fy1 = fy0 + frame_h_i64;

    let gx0 = geom_x as i64;
    let gy0 = geom_y as i64;
    let gx1 = gx0 + geom_w_i64;
    let gy1 = gy0 + geom_h_i64;

    let ix0 = fx0.max(gx0);
    let iy0 = fy0.max(gy0);
    let ix1 = fx1.min(gx1);
    let iy1 = fy1.min(gy1);

    if ix0 >= ix1 || iy0 >= iy1 {
        return None;
    }

    let src_x = (ix0 - fx0) as u32;
    let src_y = (iy0 - fy0) as u32;
    let width = (ix1 - ix0) as u32;
    let height = (iy1 - iy0) as u32;
    let dest_x = (ix0 - gx0) as i32;
    let dest_y = (iy0 - gy0) as i32;

    Some(MonitorCrop {
        src_x,
        src_y,
        width,
        height,
        dest_x,
        dest_y,
    })
}

/// Extract and convert a cropped sub-region of an RGBA image into premultiplied BGRA bytes
/// compatible with Cairo's ARgb32 format.
pub(super) fn extract_cropped_bgra(image: &image::RgbaImage, crop: &MonitorCrop) -> Vec<u8> {
    let mut bgra = Vec::with_capacity((crop.width * crop.height * 4) as usize);
    let raw = image.as_raw();
    let img_w = image.width() as usize;

    for y in 0..crop.height as usize {
        let src_row = crop.src_y as usize + y;
        let row_start = (src_row * img_w + crop.src_x as usize) * 4;
        let row_end = row_start + (crop.width as usize * 4);
        let row_slice = &raw[row_start..row_end];
        for pixel in row_slice.chunks_exact(4) {
            let alpha = pixel[3] as u16;
            let r = (pixel[0] as u16 * alpha / 255) as u8;
            let g = (pixel[1] as u16 * alpha / 255) as u8;
            let b = (pixel[2] as u16 * alpha / 255) as u8;
            bgra.push(b);
            bgra.push(g);
            bgra.push(r);
            bgra.push(pixel[3]);
        }
    }
    bgra
}

fn create_monitor_overlay(monitor: &gtk::gdk::Monitor) -> Result<MonitorOverlay, Box<dyn Error>> {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.init_layer_shell();
    window.set_monitor(monitor);
    window.set_layer(gtk_layer_shell::Layer::Overlay);
    window.set_namespace("pingloom-overlay");
    window.style_context().add_class("pingloom-overlay");
    window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::None);
    window.set_exclusive_zone(0);
    window.set_anchor(gtk_layer_shell::Edge::Top, true);
    window.set_anchor(gtk_layer_shell::Edge::Bottom, true);
    window.set_anchor(gtk_layer_shell::Edge::Left, true);
    window.set_anchor(gtk_layer_shell::Edge::Right, true);
    window.set_accept_focus(false);
    window.set_focus_on_map(false);
    window.set_can_focus(false);
    window.set_decorated(false);
    window.set_app_paintable(true);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);

    if let Some(screen) =
        gtk::prelude::GtkWindowExt::screen(&window).or_else(gtk::gdk::Screen::default)
    {
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
    }

    // Strictly enforce empty input region before first mapped commit and on realize/map
    let empty = gtk::cairo::Region::create();
    window.input_shape_combine_region(Some(&empty));

    window.connect_realize(|w| {
        let empty = gtk::cairo::Region::create();
        w.input_shape_combine_region(Some(&empty));
        if let Some(gdk_win) = w.window() {
            gdk_win.input_shape_combine_region(&empty, 0, 0);
            gdk_win.set_pass_through(true);
            gdk_win.set_accept_focus(false);
            gdk_win.set_focus_on_map(false);
        }
    });

    window.connect_map(|w| {
        let empty = gtk::cairo::Region::create();
        w.input_shape_combine_region(Some(&empty));
        if let Some(gdk_win) = w.window() {
            gdk_win.input_shape_combine_region(&empty, 0, 0);
            gdk_win.set_pass_through(true);
        }
    });

    window.realize();
    if let Some(gdk_win) = window.window() {
        let empty = gtk::cairo::Region::create();
        gdk_win.input_shape_combine_region(&empty, 0, 0);
        gdk_win.set_pass_through(true);
        gdk_win.set_accept_focus(false);
        gdk_win.set_focus_on_map(false);
    }

    let drawing_area = gtk::DrawingArea::new();
    drawing_area.style_context().add_class("pingloom-overlay");
    let current_crop = Rc::new(RefCell::new(None::<CropSurface>));
    let crop_for_draw = Rc::clone(&current_crop);

    drawing_area.connect_draw(move |_area, context| {
        let borrow = crop_for_draw.borrow();
        let _ = context.save();
        context.set_operator(gtk::cairo::Operator::Clear);
        let _ = context.paint();
        if let Some(crop_surface) = borrow.as_ref() {
            context.set_operator(gtk::cairo::Operator::Over);
            let _ = context.set_source_surface(
                &crop_surface.surface,
                crop_surface.dest_x as f64,
                crop_surface.dest_y as f64,
            );
            let _ = context.paint();
        }
        let _ = context.restore();
        gtk::glib::Propagation::Proceed
    });

    window.add(&drawing_area);
    drawing_area.show();

    Ok(MonitorOverlay {
        monitor: monitor.clone(),
        window,
        drawing_area,
        current_crop,
    })
}

fn reconcile_monitors(
    display: &gtk::gdk::Display,
    overlays: &mut Vec<MonitorOverlay>,
) -> Result<(), Box<dyn Error>> {
    let n = display.n_monitors();
    let mut current_monitors = Vec::new();
    for i in 0..n {
        if let Some(m) = display.monitor(i) {
            current_monitors.push(m);
        }
    }

    // 1. Remove overlays whose monitor is no longer in current_monitors
    overlays.retain(|overlay| {
        let still_present = current_monitors.iter().any(|m| m == &overlay.monitor);
        if !still_present {
            overlay.window.hide();
            unsafe {
                overlay.window.destroy();
            }
        }
        still_present
    });

    // 2. Add overlays for new monitors not yet tracked
    for monitor in current_monitors {
        if !overlays.iter().any(|o| o.monitor == monitor) {
            let overlay = create_monitor_overlay(&monitor)?;
            overlays.push(overlay);
        }
    }

    Ok(())
}

/// Initialize the native Wayland passive overlay module.
///
/// Must be called on the GTK main thread.
/// Detects layer-shell support, creates per-monitor overlay windows,
/// and registers hotplug callbacks for monitor add/remove.
pub(super) fn initialize() -> Result<(), Box<dyn Error>> {
    if !gtk::is_initialized() && gtk::init().is_err() {
        return Err("Failed to initialize GTK".into());
    }

    if !gtk_layer_shell::is_supported() {
        return Err("Wayland layer-shell protocol (zwlr_layer_shell_v1) is not supported or not running under a Wayland compositor".into());
    }

    let display = gtk::gdk::Display::default().ok_or("No GDK display available")?;

    let type_name = gtk::glib::object::ObjectExt::type_(&display).name();
    if type_name != "GdkWaylandDisplay" {
        return Err(format!(
            "Expected GdkWaylandDisplay, but running on {type_name}; XWayland or X11 not supported"
        )
        .into());
    }

    // Ensure application-level CSS enforces complete background transparency
    let provider = gtk::CssProvider::new();
    let _ = provider.load_from_data(
        b".pingloom-overlay { background-color: transparent; background-image: none; }",
    );
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let mut monitors = Vec::new();
    reconcile_monitors(&display, &mut monitors)?;

    // Hook monitor hotplug signals on display
    display.connect_monitor_added(|_disp, _mon| {
        STATE.with(|state| {
            if let Some(state) = state.borrow_mut().as_mut() {
                let _ = reconcile_monitors(&state.display, &mut state.monitors);
            }
        });
    });

    display.connect_monitor_removed(|_disp, _mon| {
        STATE.with(|state| {
            if let Some(state) = state.borrow_mut().as_mut() {
                let _ = reconcile_monitors(&state.display, &mut state.monitors);
            }
        });
    });

    STATE.with(|state| {
        *state.borrow_mut() = Some(WaylandState { display, monitors });
    });

    Ok(())
}

/// Present a frame across all connected Wayland monitors.
///
/// Must be called on the GTK main thread.
/// When `frame` is `Some`, computes the per-monitor cropped region in global logical
/// coordinates, uploads the bounded cropped buffer, and triggers redrawing on visible surfaces.
/// When `frame` is `None`, unmaps/hides all overlay surfaces and drops cached pixel buffers.
pub(super) fn present(frame: Option<Arc<Frame>>) -> Result<(), Box<dyn Error>> {
    STATE.with(|state| -> Result<(), Box<dyn Error>> {
        let mut borrow = state.borrow_mut();
        if borrow.is_none() {
            drop(borrow);
            initialize()?;
            borrow = state.borrow_mut();
        }

        let state = borrow.as_mut().ok_or("Wayland overlay state unavailable")?;

        // Reconcile monitors to handle any outputs plugged/unplugged between frames
        reconcile_monitors(&state.display, &mut state.monitors)?;

        match frame {
            None => {
                for overlay in &mut state.monitors {
                    *overlay.current_crop.borrow_mut() = None;
                    if overlay.window.is_visible() {
                        overlay.window.hide();
                    }
                }
            }
            Some(frame) => {
                let frame_w = frame.image.width();
                let frame_h = frame.image.height();

                for overlay in &mut state.monitors {
                    let geom = overlay.monitor.geometry();
                    let crop = compute_crop(
                        frame.x,
                        frame.y,
                        frame_w,
                        frame_h,
                        geom.x(),
                        geom.y(),
                        geom.width(),
                        geom.height(),
                    );

                    if let Some(crop) = crop {
                        let bgra = extract_cropped_bgra(&frame.image, &crop);
                        let surface = gtk::cairo::ImageSurface::create_for_data(
                            bgra,
                            gtk::cairo::Format::ARgb32,
                            crop.width as i32,
                            crop.height as i32,
                            crop.width as i32 * 4,
                        )?;

                        *overlay.current_crop.borrow_mut() = Some(CropSurface {
                            surface,
                            dest_x: crop.dest_x,
                            dest_y: crop.dest_y,
                        });

                        overlay.drawing_area.queue_draw();
                        if !overlay.window.is_visible() {
                            overlay.window.show();
                        }
                    } else {
                        *overlay.current_crop.borrow_mut() = None;
                        if overlay.window.is_visible() {
                            overlay.window.hide();
                        }
                    }
                }
            }
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_fully_contained() {
        let crop = compute_crop(100, 200, 50, 60, 0, 0, 1920, 1080).unwrap();
        assert_eq!(
            crop,
            MonitorCrop {
                src_x: 0,
                src_y: 0,
                width: 50,
                height: 60,
                dest_x: 100,
                dest_y: 200,
            }
        );
    }

    #[test]
    fn test_crop_horizontal_split() {
        let crop_left = compute_crop(1900, 100, 100, 50, 0, 0, 1920, 1080).unwrap();
        assert_eq!(
            crop_left,
            MonitorCrop {
                src_x: 0,
                src_y: 0,
                width: 20,
                height: 50,
                dest_x: 1900,
                dest_y: 100,
            }
        );

        let crop_right = compute_crop(1900, 100, 100, 50, 1920, 0, 1920, 1080).unwrap();
        assert_eq!(
            crop_right,
            MonitorCrop {
                src_x: 20,
                src_y: 0,
                width: 80,
                height: 50,
                dest_x: 0,
                dest_y: 100,
            }
        );
    }

    #[test]
    fn test_crop_vertical_split() {
        let crop_top = compute_crop(100, 1050, 50, 100, 0, 0, 1920, 1080).unwrap();
        assert_eq!(
            crop_top,
            MonitorCrop {
                src_x: 0,
                src_y: 0,
                width: 50,
                height: 30,
                dest_x: 100,
                dest_y: 1050,
            }
        );

        let crop_bottom = compute_crop(100, 1050, 50, 100, 0, 1080, 1920, 1080).unwrap();
        assert_eq!(
            crop_bottom,
            MonitorCrop {
                src_x: 0,
                src_y: 30,
                width: 50,
                height: 70,
                dest_x: 100,
                dest_y: 0,
            }
        );
    }

    #[test]
    fn test_crop_no_intersection() {
        assert_eq!(compute_crop(2000, 100, 50, 50, 0, 0, 1920, 1080), None);
        assert_eq!(compute_crop(-100, 100, 50, 50, 0, 0, 1920, 1080), None);
        assert_eq!(compute_crop(100, 1200, 50, 50, 0, 0, 1920, 1080), None);
        assert_eq!(compute_crop(100, -100, 50, 50, 0, 0, 1920, 1080), None);
        assert_eq!(compute_crop(100, 100, 0, 50, 0, 0, 1920, 1080), None);
    }

    #[test]
    fn test_crop_negative_monitor_coordinates() {
        let crop_left = compute_crop(-100, 200, 200, 200, -1920, 0, 1920, 1080).unwrap();
        assert_eq!(
            crop_left,
            MonitorCrop {
                src_x: 0,
                src_y: 0,
                width: 100,
                height: 200,
                dest_x: 1820,
                dest_y: 200,
            }
        );

        let crop_right = compute_crop(-100, 200, 200, 200, 0, 0, 1920, 1080).unwrap();
        assert_eq!(
            crop_right,
            MonitorCrop {
                src_x: 100,
                src_y: 0,
                width: 100,
                height: 200,
                dest_x: 0,
                dest_y: 200,
            }
        );
    }

    #[test]
    fn test_crop_quad_split() {
        let tl = compute_crop(950, 950, 100, 100, 0, 0, 1000, 1000).unwrap();
        assert_eq!(
            tl,
            MonitorCrop {
                src_x: 0,
                src_y: 0,
                width: 50,
                height: 50,
                dest_x: 950,
                dest_y: 950,
            }
        );

        let tr = compute_crop(950, 950, 100, 100, 1000, 0, 1000, 1000).unwrap();
        assert_eq!(
            tr,
            MonitorCrop {
                src_x: 50,
                src_y: 0,
                width: 50,
                height: 50,
                dest_x: 0,
                dest_y: 950,
            }
        );

        let bl = compute_crop(950, 950, 100, 100, 0, 1000, 1000, 1000).unwrap();
        assert_eq!(
            bl,
            MonitorCrop {
                src_x: 0,
                src_y: 50,
                width: 50,
                height: 50,
                dest_x: 950,
                dest_y: 0,
            }
        );

        let br = compute_crop(950, 950, 100, 100, 1000, 1000, 1000, 1000).unwrap();
        assert_eq!(
            br,
            MonitorCrop {
                src_x: 50,
                src_y: 50,
                width: 50,
                height: 50,
                dest_x: 0,
                dest_y: 0,
            }
        );
    }

    #[test]
    fn test_extract_cropped_bgra() {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, image::Rgba([0, 255, 0, 128]));
        img.put_pixel(0, 1, image::Rgba([0, 0, 255, 255]));
        img.put_pixel(1, 1, image::Rgba([255, 255, 255, 0]));

        let crop = MonitorCrop {
            src_x: 0,
            src_y: 0,
            width: 2,
            height: 2,
            dest_x: 10,
            dest_y: 20,
        };
        let bgra = extract_cropped_bgra(&img, &crop);
        assert_eq!(bgra.len(), 16);
        // (0, 0): Red (255, 0, 0, 255) -> BGRA [0, 0, 255, 255]
        assert_eq!(&bgra[0..4], &[0, 0, 255, 255]);
        // (1, 0): Green (0, 255, 0, 128) -> G premul = 255*128/255 = 128 -> BGRA [0, 128, 0, 128]
        assert_eq!(&bgra[4..8], &[0, 128, 0, 128]);
        // (0, 1): Blue (0, 0, 255, 255) -> BGRA [255, 0, 0, 255]
        assert_eq!(&bgra[8..12], &[255, 0, 0, 255]);
        // (1, 1): Alpha 0 -> BGRA [0, 0, 0, 0]
        assert_eq!(&bgra[12..16], &[0, 0, 0, 0]);
    }
}
