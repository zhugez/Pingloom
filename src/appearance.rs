//! Appearance is immediate and independent of the native gesture-settings draft.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        OnceLock,
    },
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub(crate) enum Theme {
    DefaultLight,
    #[default]
    DefaultDark,
    Latte,
    Frappe,
    Macchiato,
    Mocha,
}
static CURRENT: AtomicU8 = AtomicU8::new(Theme::DefaultDark as u8);
static PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub hub: [u8; 3],
    pub line: [u8; 3],
    pub inner: [u8; 3],
    pub outer: [u8; 3],
    pub ink: [u8; 3],
    pub accent: [u8; 3],
}
impl Theme {
    pub(crate) fn current() -> Self {
        match CURRENT.load(Ordering::Acquire) {
            0 => Self::DefaultLight,
            2 => Self::Latte,
            3 => Self::Frappe,
            4 => Self::Macchiato,
            5 => Self::Mocha,
            _ => Self::DefaultDark,
        }
    }
    pub(crate) fn palette(self) -> Palette {
        match self {
            Self::DefaultLight => Palette {
                hub: [255, 255, 255],
                line: [233, 223, 224],
                inner: [255, 255, 255],
                outer: [247, 232, 233],
                ink: [107, 96, 100],
                accent: [226, 180, 189],
            },
            Self::DefaultDark => Palette {
                hub: [26, 23, 27],
                line: [61, 52, 60],
                inner: [27, 23, 28],
                outer: [40, 35, 42],
                ink: [189, 177, 180],
                accent: [226, 180, 189],
            },
            Self::Latte => Palette {
                hub: [230, 233, 239],
                line: [204, 208, 218],
                inner: [230, 233, 239],
                outer: [239, 241, 245],
                ink: [92, 95, 119],
                accent: [32, 159, 181],
            },
            Self::Frappe => Palette {
                hub: [41, 44, 60],
                line: [65, 69, 89],
                inner: [41, 44, 60],
                outer: [48, 52, 70],
                ink: [181, 191, 226],
                accent: [133, 193, 220],
            },
            Self::Macchiato => Palette {
                hub: [30, 32, 48],
                line: [54, 58, 79],
                inner: [30, 32, 48],
                outer: [36, 39, 58],
                ink: [184, 192, 224],
                accent: [125, 196, 228],
            },
            Self::Mocha => Palette {
                hub: [24, 24, 37],
                line: [49, 50, 68],
                inner: [24, 24, 37],
                outer: [30, 30, 46],
                ink: [186, 194, 222],
                accent: [116, 199, 236],
            },
        }
    }
}

pub(crate) fn initialize(settings: &Path) {
    let path = settings.with_file_name("appearance.json");
    if PATH.set(path.clone()).is_ok() {
        match load(&path) {
            Ok(theme) => CURRENT.store(theme as u8, Ordering::Release),
            Err(error) => eprintln!("Could not load appearance; using Night: {error}"),
        }
    }
}
pub(crate) fn set(theme: Theme) -> Result<(), String> {
    let path = PATH.get().ok_or("Appearance storage is not ready")?;
    persist(path, theme).map_err(|e| format!("Could not save appearance: {e}"))?;
    CURRENT.store(theme as u8, Ordering::Release);
    Ok(())
}
fn load(path: &Path) -> Result<Theme, Box<dyn std::error::Error>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Theme::default()),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(65).read_to_end(&mut bytes)?;
    if bytes.len() > 64 {
        return Err("appearance file exceeds 64 bytes".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn persist(path: &Path, theme: Theme) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("Appearance directory unavailable")?;
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&serde_json::to_vec(&theme)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_known_palettes_persist_without_changing_gesture_settings() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings.json");
        std::fs::write(&settings, b"original gesture settings").unwrap();
        let path = settings.with_file_name("appearance.json");
        assert_eq!(load(&path).unwrap(), Theme::DefaultDark);
        for (name, theme) in [
            ("default-light", Theme::DefaultLight),
            ("default-dark", Theme::DefaultDark),
            ("latte", Theme::Latte),
            ("frappe", Theme::Frappe),
            ("macchiato", Theme::Macchiato),
            ("mocha", Theme::Mocha),
        ] {
            assert_eq!(
                serde_json::from_str::<Theme>(&format!("\"{name}\"")).unwrap(),
                theme
            );
            persist(&path, theme).unwrap();
            assert_eq!(load(&path).unwrap(), theme);
        }
        for invalid in ["\"../../settings.json\"", "\"unknown\"", "42", "null", "{}"] {
            assert!(serde_json::from_str::<Theme>(invalid).is_err());
        }
        std::fs::write(&path, vec![b'x'; 65]).unwrap();
        assert!(load(&path).is_err());
        assert_eq!(
            std::fs::read(&settings).unwrap(),
            b"original gesture settings"
        );
    }
}
