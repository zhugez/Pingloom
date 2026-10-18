use crate::model::Point;

pub const WHEEL_INNER_RADIUS: f64 = 86.0;

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

/// Selects with the wheel's DIP dead zone scaled to physical pixels at its origin display.
pub fn select_slot_scaled(origin: Point, point: Point, scale: f64) -> Option<usize> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let dx = i64::from(point.x) - i64::from(origin.x);
    let dy = i64::from(point.y) - i64::from(origin.y);
    pick_slice(dx as f64, dy as f64, WHEEL_INNER_RADIUS * scale)
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
}
