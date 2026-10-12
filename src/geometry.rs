use crate::model::Point;

pub const WHEEL_INNER_RADIUS: f64 = 86.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointF {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn valid(self) -> bool {
        self.width > 0 && self.height > 0
    }

    fn contains(self, point: Point) -> bool {
        let (x, y) = (i64::from(point.x), i64::from(point.y));
        let right = i64::from(self.x) + i64::from(self.width);
        let bottom = i64::from(self.y) + i64::from(self.height);
        x >= i64::from(self.x) && x < right && y >= i64::from(self.y) && y < bottom
    }

    fn distance_squared(self, point: Point) -> i128 {
        let (x, y) = (i64::from(point.x), i64::from(point.y));
        let right = i64::from(self.x) + i64::from(self.width) - 1;
        let bottom = i64::from(self.y) + i64::from(self.height) - 1;
        let dx = (i64::from(self.x) - x).max(0).max(x - right) as i128;
        let dy = (i64::from(self.y) - y).max(0).max(y - bottom) as i128;
        dx * dx + dy * dy
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectF {
    fn valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayMap {
    pub id: u64,
    pub physical: Rect,
    pub dip: RectF,
    pub scale: f64,
}

impl DisplayMap {
    fn valid(self) -> bool {
        self.physical.valid() && self.dip.valid() && self.scale.is_finite() && self.scale > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MappedPoint {
    pub display_id: u64,
    pub x: f64,
    pub y: f64,
}

/// Clockwise wheel slice index from north, or None inside the dead zone / for invalid input.
pub fn pick_slice(dx: f64, dy: f64, dead_zone: f64) -> Option<usize> {
    if !dx.is_finite() || !dy.is_finite() || !dead_zone.is_finite() || dead_zone < 0.0 {
        return None;
    }
    if dx.hypot(dy) < dead_zone {
        return None;
    }
    let degrees = ((dy.atan2(dx).to_degrees() + 90.0 + 22.5 + 360.0) % 360.0).floor();
    Some((degrees / 45.0) as usize % 8)
}

pub fn select_slot(origin: Point, point: Point) -> Option<usize> {
    select_slot_scaled(origin, point, 1.0)
}

/// Selects with the wheel's DIP dead zone scaled to physical pixels at its origin display.
pub fn select_slot_scaled(origin: Point, point: Point, scale: f64) -> Option<usize> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let dx = i64::from(point.x) - i64::from(origin.x);
    let dy = i64::from(point.y) - i64::from(origin.y);
    pick_slice(dx as f64, dy as f64, WHEEL_INNER_RADIUS * scale)
}

/// Finds the monitor containing the physical point, or the closest valid monitor if outside all displays.
pub fn display_at_physical(point: Point, maps: &[DisplayMap]) -> Option<&DisplayMap> {
    maps.iter()
        .filter(|map| map.valid())
        .find(|map| map.physical.contains(point))
        .or_else(|| {
            maps.iter()
                .filter(|map| map.valid())
                .min_by_key(|map| map.physical.distance_squared(point))
        })
}

/// Converts global physical screen pixels to global DIPs using the monitor under (or nearest) the point.
pub fn physical_to_dip(point: Point, maps: &[DisplayMap]) -> Option<MappedPoint> {
    let display = display_at_physical(point, maps)?;
    let x = display.dip.x + (f64::from(point.x) - f64::from(display.physical.x)) / display.scale;
    let y = display.dip.y + (f64::from(point.y) - f64::from(display.physical.y)) / display.scale;
    (x.is_finite() && y.is_finite()).then_some(MappedPoint {
        display_id: display.id,
        x,
        y,
    })
}

/// Converts to CSS/DIP pixels local to the selected display's overlay.
pub fn physical_to_local(
    point: Point,
    maps: &[DisplayMap],
    display_id: Option<u64>,
) -> Option<MappedPoint> {
    let global = physical_to_dip(point, maps)?;
    let target = match display_id {
        Some(id) => maps.iter().find(|map| map.id == id && map.valid())?,
        None => maps
            .iter()
            .find(|map| map.id == global.display_id && map.valid())?,
    };
    let x = global.x - target.dip.x;
    let y = global.y - target.dip.y;
    (x.is_finite() && y.is_finite()).then_some(MappedPoint {
        display_id: target.id,
        x,
        y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_clockwise(degrees: f64, radius: f64) -> Option<usize> {
        let radians = (degrees - 90.0).to_radians();
        pick_slice(
            radius * radians.cos(),
            radius * radians.sin(),
            WHEEL_INNER_RADIUS,
        )
    }

    fn map(id: u64, physical: Rect, dip: RectF, scale: f64) -> DisplayMap {
        DisplayMap {
            id,
            physical,
            dip,
            scale,
        }
    }

    #[test]
    fn maps_compass_directions_and_center_dead_zone() {
        assert_eq!(
            (0..8)
                .map(|i| at_clockwise(i as f64 * 45.0, 150.0).unwrap())
                .collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        assert_eq!(
            pick_slice(0.0, -(WHEEL_INNER_RADIUS - 1.0), WHEEL_INNER_RADIUS),
            None
        );
        assert_eq!(
            pick_slice(0.0, -WHEEL_INNER_RADIUS, WHEEL_INNER_RADIUS),
            Some(0)
        );
        assert_eq!(at_clockwise(22.4, 150.0), Some(0));
        assert_eq!(at_clockwise(22.6, 150.0), Some(1));
        assert_eq!(at_clockwise(359.9, 150.0), Some(0));
        assert_eq!(pick_slice(f64::NAN, 0.0, 86.0), None);
        assert_eq!(
            select_slot_scaled(Point::new(0, 0), Point::new(100, 0), 1.5),
            None
        );
        assert_eq!(
            select_slot_scaled(Point::new(0, 0), Point::new(129, 0), 1.5),
            Some(2)
        );
        assert_eq!(
            select_slot_scaled(Point::new(0, 0), Point::new(129, 0), f64::NAN),
            None
        );
    }

    #[test]
    fn converts_negative_monitor_coordinates_and_mixed_dpi() {
        let maps = [
            map(
                1,
                Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                RectF {
                    x: 0.0,
                    y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                1.0,
            ),
            map(
                2,
                Rect {
                    x: -2560,
                    y: -200,
                    width: 2560,
                    height: 1440,
                },
                RectF {
                    x: -1706.6666666667,
                    y: -133.3333333333,
                    width: 1706.6666666667,
                    height: 960.0,
                },
                1.5,
            ),
        ];
        let physical = Point::new(-1060, 550);
        let global = physical_to_dip(physical, &maps).unwrap();
        assert_eq!(global.display_id, 2);
        assert!((global.x - (-706.6666666667)).abs() < 0.0001);
        assert!((global.y - 366.6666666667).abs() < 0.0001);
        let local = physical_to_local(physical, &maps, None).unwrap();
        assert_eq!(local.display_id, 2);
        assert!((local.x - 1000.0).abs() < 0.0001);
        assert!((local.y - 500.0).abs() < 0.0001);
    }

    #[test]
    fn points_outside_monitors_use_the_nearest_valid_display() {
        let maps = [
            map(
                1,
                Rect {
                    x: -100,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                RectF {
                    x: -100.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                1.0,
            ),
            map(
                2,
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                RectF {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                1.0,
            ),
            map(
                3,
                Rect {
                    x: i32::MAX,
                    y: i32::MAX,
                    width: 0,
                    height: 0,
                },
                RectF {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                1.0,
            ),
        ];
        assert_eq!(
            display_at_physical(Point::new(-120, 50), &maps).unwrap().id,
            1
        );
        assert_eq!(
            physical_to_local(Point::new(-120, 50), &maps, Some(2))
                .unwrap()
                .x,
            -120.0
        );
        assert!(physical_to_local(Point::new(10, 10), &maps, Some(99)).is_none());
        let tiny_scale = [map(
            4,
            Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            RectF {
                x: 0.0,
                y: 0.0,
                width: 2.0,
                height: 2.0,
            },
            f64::MIN_POSITIVE / 8.0,
        )];
        assert!(physical_to_dip(Point::new(1, 1), &tiny_scale).is_none());
        assert!(display_at_physical(Point::new(0, 0), &[]).is_none());
    }
}
