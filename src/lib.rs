//! Portable input decisions, wheel geometry, asset identities, and settings for lolping-rs.

pub mod geometry;
pub mod gesture;
pub mod model;
pub mod settings;

pub use geometry::{DisplayMap, PointF, Rect, RectF};
pub use gesture::{GestureEngine, InputDecision};
pub use model::{Action, Button, EmoteId, InputEvent, PingId, Point, Trigger, WheelKind};
pub use settings::{Hotkey, Settings, SettingsError, ToggleMods};
