use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PingId {
    Danger,
    Push,
    #[serde(rename = "omw")]
    OnMyWay,
    #[serde(rename = "allin")]
    AllIn,
    Assist,
    #[serde(rename = "needvision")]
    NeedVision,
    Missing,
    #[serde(rename = "enemyvision")]
    EnemyVision,
    Generic,
    Bait,
    #[serde(rename = "visioncleared")]
    VisionCleared,
}

impl PingId {
    pub const ALL: [Self; 11] = [
        Self::Danger,
        Self::Push,
        Self::OnMyWay,
        Self::AllIn,
        Self::Assist,
        Self::NeedVision,
        Self::Missing,
        Self::EnemyVision,
        Self::Generic,
        Self::Bait,
        Self::VisionCleared,
    ];

    pub const fn asset(self) -> &'static PingAsset {
        &PING_ASSETS[self as usize]
    }
}

pub const DEFAULT_PING_WHEEL: [PingId; 8] = [
    PingId::Danger,
    PingId::Push,
    PingId::OnMyWay,
    PingId::AllIn,
    PingId::Assist,
    PingId::NeedVision,
    PingId::Missing,
    PingId::EnemyVision,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PingAsset {
    pub id: PingId,
    pub name: &'static str,
    pub icon: &'static str,
    pub art: &'static [&'static str],
    pub sound: &'static str,
    pub color: &'static str,
}

pub const PING_ASSETS: [PingAsset; 11] = [
    PingAsset {
        id: PingId::Danger,
        name: "Danger",
        icon: "pingwheel_retreatrender",
        art: &["pingwheel_retreatrender"],
        sound: "SRP_12",
        color: "#ff3b55",
    },
    PingAsset {
        id: PingId::Push,
        name: "Push",
        icon: "push",
        art: &["pingwheel_pushrender"],
        sound: "SRP_9",
        color: "#35d27a",
    },
    PingAsset {
        id: PingId::OnMyWay,
        name: "On My Way",
        icon: "omw",
        art: &["omw"],
        sound: "OnMyWay",
        color: "#2e8bff",
    },
    PingAsset {
        id: PingId::AllIn,
        name: "All In",
        icon: "all_in",
        art: &["pingwheel_allin_render_01", "pingwheel_allin_render_02"],
        sound: "SRP_4",
        color: "#f2b320",
    },
    PingAsset {
        id: PingId::Assist,
        name: "Assist Me",
        icon: "assist_me",
        art: &["assist_me"],
        sound: "ComeHere",
        color: "#56d43c",
    },
    PingAsset {
        id: PingId::NeedVision,
        name: "Need Vision",
        icon: "need_vision",
        art: &["need_vision"],
        sound: "SRP_7",
        color: "#56d43c",
    },
    PingAsset {
        id: PingId::Missing,
        name: "Enemy Missing",
        icon: "enemy_missing",
        art: &["enemy_missing"],
        sound: "MIA",
        color: "#f2c518",
    },
    PingAsset {
        id: PingId::EnemyVision,
        name: "Enemy Vision",
        icon: "enemy_vision",
        art: &["pingwheel_enemyvisionrender"],
        sound: "AreaIsWarded",
        color: "#ff3b55",
    },
    PingAsset {
        id: PingId::Generic,
        name: "Generic",
        icon: "generic_ping",
        art: &["pingwheel_basicpingrender"],
        sound: "Base",
        color: "#2e8bff",
    },
    PingAsset {
        id: PingId::Bait,
        name: "Bait",
        icon: "pingwheel_baitrender",
        art: &["pingwheel_baitrender"],
        sound: "SRP_11",
        color: "#efbc12",
    },
    PingAsset {
        id: PingId::VisionCleared,
        name: "Vision Cleared",
        icon: "vision_cleared",
        art: &["pingwheel_visionclearedrender"],
        sound: "SRP_6",
        color: "#2b80ff",
    },
];

pub const DEFAULT_CLICK_PING: PingId = PingId::Generic;
pub const TICK_SOUND: &str = "button";

/// Built-in League emote asset slugs; each has `<slug>.png`, `<slug>.webp`, and `<slug>.ogg`.
/// The slugs are persisted as settings identities and are also the on-disk asset names.
pub const EMOTE_ASSETS: &[&str] = &[
    "facepalm",
    "unworthy",
    "unworthy_cn",
    "poro_snax",
    "sad_kitten",
    "cheeky_poro",
    "angry_kitty",
    "gg_heart",
    "bee_happy",
    "bee_mad",
    "bee_sad",
    "snoozy_poro",
    "wahaha",
    "omg_love_it",
    "poro_ride",
    "dont_make_me_laugh",
    "woah_dizzy",
    "haha_hilarious",
    "gasp",
    "super_approved",
    "pain",
    "laughing_out_loud",
    "hats_off",
    "call_me",
    "think_about_it",
    "unimpressed",
    "did_you_just",
    "nice_try",
    "nice",
    "despair",
    "does_not_compute",
    "come_at_me",
    "how_could_you",
    "scout_approved",
    "maybe_next_time",
    "okay",
    "good_job_buddy",
    "teamwork",
    "i_will_destroy_you",
    "not_today",
    "oh",
    "come_again",
    "magpie_oopsie",
    "such_foolishness",
    "now_see_here",
    "let_s_do_this",
    "long_road_ahead",
    "sweating_blitzcrank",
];

pub const DEFAULT_EMOTE_WHEEL: [&str; 8] = [
    "come_at_me",
    "now_see_here",
    "does_not_compute",
    "sweating_blitzcrank",
    "unworthy_cn",
    "gg_heart",
    "bee_happy",
    "bee_mad",
];
pub const DEFAULT_CLICK_EMOTE: &str = "facepalm";

pub type EmoteId = String;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Alt,
    Ctrl,
    Shift,
    Win,
    CapsLock,
    Mouse4,
    Mouse5,
    Key { vk: u8 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WheelKind {
    Ping,
    Emote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Button {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    KeyDown {
        vk: u8,
        injected: bool,
    },
    KeyUp {
        vk: u8,
        injected: bool,
    },
    PointerDown {
        button: Button,
        point: Point,
        time_ms: u64,
        injected: bool,
        /// True when a fullscreen app owns this point; only consulted for a gesture candidate.
        fullscreen_app: bool,
        /// Physical pixels per DIP on the display containing this gesture origin.
        dpi_scale: f64,
    },
    PointerUp {
        button: Button,
        point: Point,
        time_ms: u64,
        injected: bool,
    },
    PointerMove {
        point: Point,
        time_ms: u64,
        injected: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Action {
    WheelOpen {
        wheel: WheelKind,
        origin: Point,
    },
    WheelMove {
        wheel: WheelKind,
        point: Point,
        slot: Option<usize>,
    },
    Select {
        wheel: WheelKind,
        slot: usize,
        point: Point,
    },
    Click {
        wheel: WheelKind,
        point: Point,
    },
    Cancel,
    Toggled {
        enabled: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_ping_texture_and_sound_is_referenced() {
        let textures: std::collections::HashSet<_> = PING_ASSETS
            .iter()
            .flat_map(|a| std::iter::once(a.icon).chain(a.art.iter().copied()))
            .chain(["pingloom-wheel-labels"])
            .collect();
        let sounds: std::collections::HashSet<_> = PING_ASSETS
            .iter()
            .map(|a| a.sound)
            .chain([TICK_SOUND])
            .collect();
        for (directory, used) in [("textures", textures), ("sounds", sounds)] {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join(directory);
            let orphans: Vec<_> = std::fs::read_dir(root)
                .unwrap()
                .map(Result::unwrap)
                .filter(|e| e.file_type().unwrap().is_file())
                .filter(|e| !used.contains(e.path().file_stem().unwrap().to_str().unwrap()))
                .map(|e| e.file_name())
                .collect();
            assert!(orphans.is_empty(), "unreferenced {directory}: {orphans:?}");
        }
    }

    #[test]
    fn asset_identity_tables_match_enum_order_and_default_assignments() {
        assert!(PingId::ALL
            .iter()
            .enumerate()
            .all(|(i, id)| PING_ASSETS[i].id == *id));
        assert_eq!(DEFAULT_PING_WHEEL[0], PingId::Danger);
        assert!(DEFAULT_EMOTE_WHEEL
            .iter()
            .all(|id| EMOTE_ASSETS.contains(id)));
        assert!(DEFAULT_CLICK_EMOTE == EMOTE_ASSETS[0]);
        assert!(PING_ASSETS.iter().all(|asset| !asset.art.is_empty()));
    }
}
