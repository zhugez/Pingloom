//! V7's native Night wheel. Cache geometry per open; only color blending runs per frame.
use image::RgbaImage;
use pingloom_core::geometry::WHEEL_INNER_RADIUS;
use std::f64::consts::{FRAC_PI_4, FRAC_PI_8, TAU};

pub(crate) const EXTENT: f64 = 230.0;
pub(crate) const ICON_RADIUS: f64 = 153.0;
const LABELS: &str = "textures/pingloom-wheel-labels.png";
const ACCENT: [u8; 3] = [226, 180, 189];

// Rasterize the font atlas to the display scale once, not on every owner-thread frame.
pub(crate) fn labels(scale: f64) -> Option<RgbaImage> {
    let bytes = crate::assets::ASSETS.get_file(LABELS)?.contents();
    let atlas = image::load_from_memory(bytes).ok()?.into_rgba8();
    Some(crate::raster::resize_art(&atlas, (EXTENT * 2.0 * scale).round() as u32).into_owned())
}

#[derive(Clone, Copy)]
struct Texel {
    idle: [u8; 4],
    selected: [u8; 4],
    slot: u8,
}
impl Texel {
    fn fixed(rgba: [u8; 4]) -> Self {
        Self {
            idle: rgba,
            selected: rgba,
            slot: 0,
        }
    }
    fn rgba(self, selection: &[f64; 8]) -> [u8; 4] {
        if self.idle == self.selected {
            return self.idle;
        }
        let amount = selection[usize::from(self.slot)].clamp(0.0, 1.0);
        if amount == 0.0 {
            return self.idle;
        }
        if amount == 1.0 {
            return self.selected;
        }
        std::array::from_fn(|i| {
            (f64::from(self.idle[i])
                + (f64::from(self.selected[i]) - f64::from(self.idle[i])) * amount)
                .round() as u8
        })
    }
}

pub(crate) struct Surface {
    extent: i32,
    pixels: Vec<Texel>,
}
impl Surface {
    pub(crate) fn new(scale: f64) -> Self {
        let extent = (EXTENT * scale).ceil() as i32;
        let pixels = (-extent..=extent)
            .flat_map(|y| {
                (-extent..=extent)
                    .map(move |x| texel(f64::from(x) / scale, f64::from(y) / scale, scale))
            })
            .collect();
        Self { extent, pixels }
    }
    /// Straight RGBA for either the Win32 premultiplied canvas or Unix compositor.
    pub(crate) fn paint(&self, selection: &[f64; 8], mut point: impl FnMut(i32, i32, [u8; 4])) {
        let width = (self.extent * 2 + 1) as usize;
        for (i, texel) in self.pixels.iter().enumerate() {
            let rgba = texel.rgba(selection);
            if rgba[3] != 0 {
                point(
                    (i % width) as i32 - self.extent,
                    (i / width) as i32 - self.extent,
                    rgba,
                );
            }
        }
    }
}
fn color(rgb: [u8; 3], coverage: f64, alpha: f64) -> [u8; 4] {
    [
        rgb[0],
        rgb[1],
        rgb[2],
        (coverage.clamp(0.0, 1.0) * alpha).round() as u8,
    ]
}
fn texel(x: f64, y: f64, scale: f64) -> Texel {
    let radius = x.hypot(y);
    let aa = 0.5 / scale;
    let edge = |distance: f64| (distance + aa) * scale;
    if radius > EXTENT + aa {
        return Texel::fixed([0; 4]);
    }
    // Separate circular hub and spaced wedges, rather than the old solid disk/red X.
    if radius <= WHEEL_INNER_RADIUS + aa {
        let rgb = if radius > WHEEL_INNER_RADIUS - 1.0 - aa {
            [61, 52, 60]
        } else {
            [26, 23, 27]
        };
        return Texel::fixed(color(rgb, edge(WHEEL_INNER_RADIUS - radius), 250.0));
    }
    let angle = x.atan2(-y).rem_euclid(TAU);
    let slot = ((angle / FRAC_PI_4).round() as usize) % 8;
    let delta = ((angle - slot as f64 * FRAC_PI_4 + std::f64::consts::PI).rem_euclid(TAU)
        - std::f64::consts::PI)
        .abs();
    if radius < 93.0 && delta < FRAC_PI_8 - 0.08 {
        return Texel {
            idle: color(ACCENT, 0.0, 0.0),
            selected: color(ACCENT, edge(1.2 - (radius - 90.0).abs()), 210.0),
            slot: slot as u8,
        };
    }
    let gap = (FRAC_PI_8 - delta).sin() * radius - 2.5;
    let wedge = edge((radius - 98.0).min(220.0 - radius).min(gap));
    if wedge > 0.0 {
        let radial = ((radius - 98.0) / 122.0).clamp(0.0, 1.0);
        let base = [
            27.0 + 13.0 * radial,
            23.0 + 12.0 * radial,
            28.0 + 14.0 * radial,
        ];
        let border = edge(1.0 - (radius - 98.0).min(220.0 - radius).min(gap)).clamp(0.0, 1.0);
        let tint = |amount: f64| {
            std::array::from_fn(|i| {
                (base[i] * (1.0 - amount) + f64::from(ACCENT[i]) * amount).round() as u8
            })
        };
        let idle = color(tint(0.08 * border), wedge, 250.0);
        let mut selected = tint(0.08 * border + 0.14);
        if radius > 212.0 && delta < FRAC_PI_8 - 0.025 {
            let arc = edge(1.5 - (radius - 216.5).abs()).clamp(0.0, 1.0);
            selected = std::array::from_fn(|i| {
                (f64::from(selected[i]) * (1.0 - arc) + f64::from(ACCENT[i]) * arc).round() as u8
            });
        }
        return Texel {
            idle,
            selected: color(selected, wedge, 250.0),
            slot: slot as u8,
        };
    }
    // Broken outer bezel with fine ticks and eight longer compass ticks.
    let major = edge(0.9 - delta * 228.0).min(edge(3.5 - (radius - 227.0).abs()));
    if major > 0.0 {
        return Texel::fixed(color(ACCENT, major, 155.0));
    }
    let tick = ((angle * 228.0 + 5.0).rem_euclid(10.0) - 5.0).abs();
    Texel::fixed(color(
        [189, 177, 180],
        edge(0.6 - tick).min(edge(1.4 - (radius - 227.0).abs())),
        65.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pingloom_core::geometry::pick_slice;

    #[test]
    fn v7_wheel_has_spaced_wedges_blush_highlights_and_unchanged_dead_zone() {
        for scale in [1.0, 1.5, 2.0] {
            let sample = |x, y, selection: &[f64; 8]| texel(x, y, scale).rgba(selection);
            let idle = [0.0; 8];
            assert_eq!(sample(0.0, 0.0, &idle), [26, 23, 27, 250]);
            assert_eq!(sample(0.0, -94.0, &idle)[3], 0, "hub/wedge gap");
            let boundary = FRAC_PI_8;
            assert_eq!(
                sample(153.0 * boundary.sin(), -153.0 * boundary.cos(), &idle)[3],
                0,
                "slice gap"
            );
            assert_eq!(sample(240.0, 0.0, &idle)[3], 0);
            let mut selected = idle;
            selected[0] = 1.0;
            assert_eq!(&sample(0.0, -216.5, &selected)[..3], &ACCENT);
            assert_ne!(sample(0.0, -153.0, &selected), sample(0.0, -153.0, &idle));
            assert_eq!(sample(153.0, 0.0, &selected), sample(153.0, 0.0, &idle));
            assert_eq!(
                pick_slice(0.0, -85.0 * scale, WHEEL_INNER_RADIUS * scale),
                None
            );
            assert_eq!(
                pick_slice(0.0, -86.0 * scale, WHEEL_INNER_RADIUS * scale),
                Some(0)
            );
            // Rendering uses the same clockwise compass slots as actual input.
            for i in 0..360 {
                let a = f64::from(i).to_radians();
                let (x, y) = (153.0 * a.sin(), -153.0 * a.cos());
                let pixel = texel(x, y, scale);
                if pixel.idle[3] != 0 {
                    assert_eq!(usize::from(pixel.slot), pick_slice(x, y, 0.0).unwrap());
                }
            }
        }
    }
}
