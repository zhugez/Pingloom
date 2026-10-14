use super::{runtime_mailbox::OwnerError, Shared};
use pingloom_core::gesture::ReplayInput;
use pingloom_core::{Settings, Trigger};
use std::sync::Arc;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;

pub(super) fn run(shared: Arc<Shared>) -> Result<(), String> {
    platform::run(shared)
}
pub(super) fn replay(events: &[ReplayInput]) -> usize {
    let mut count = 0;
    for &event in events {
        if cfg!(target_os = "macos")
            && matches!(event, ReplayInput::KeyDown(0xfc) | ReplayInput::KeyUp(0xfc))
        {
            count += 1;
            continue;
        }
        if platform::replay(event).is_err() {
            break;
        }
        count += 1;
    }
    count
}
pub(crate) fn supported_vk(vk: u8) -> bool {
    platform::supported_vk(vk)
}
pub(super) fn validate(settings: &Settings) -> Result<(), OwnerError> {
    for (field, trigger) in [
        ("trigger", Some(settings.trigger)),
        ("emote_trigger", settings.emote_trigger),
    ] {
        let supported = match trigger {
            Some(Trigger::Key { vk }) => supported_vk(vk),
            Some(Trigger::Mouse4 | Trigger::Mouse5) => cfg!(target_os = "macos"),
            Some(Trigger::CapsLock) => cfg!(target_os = "linux"),
            _ => true,
        };
        if !supported {
            return Err(OwnerError {
                code: "invalid_field",
                field: Some(field.into()),
                message: "This trigger is not supported by the native input backend on this OS."
                    .into(),
            });
        }
    }
    if !supported_vk(settings.toggle_hotkey.vk) {
        return Err(OwnerError {
            code: "invalid_field",
            field: Some("toggle_hotkey.vk".into()),
            message: "This hotkey key is not supported on this OS.".into(),
        });
    }
    Ok(())
}
