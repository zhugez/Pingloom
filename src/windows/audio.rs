use super::assets;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    io::Cursor,
    sync::mpsc::{self, RecvTimeoutError, SyncSender},
    thread,
    time::{Duration, Instant},
};

enum Message {
    Warm,
    Play(String, f32),
}
pub struct Audio {
    sender: SyncSender<Message>,
}
const IDLE: Duration = Duration::from_secs(30);
const RETRY: Duration = Duration::from_secs(5);
fn may_open(now: Instant, failed: Option<Instant>) -> bool {
    failed.is_none_or(|at| now.saturating_duration_since(at) >= RETRY)
}
fn may_close(now: Instant, last_play: Instant, voices: usize) -> bool {
    voices == 0 && now.saturating_duration_since(last_play) >= IDLE
}
impl Audio {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(32);
        thread::Builder::new()
            .name("pingloom-audio".into())
            .spawn(move || {
                let mut output: Option<(OutputStream, OutputStreamHandle)> = None;
                let mut voices: Vec<Sink> = Vec::new();
                let mut failed = None;
                let mut last_play = Instant::now();
                loop {
                    let message = match receiver.recv_timeout(Duration::from_secs(1)) {
                        Ok(message) => Some(message),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    voices.retain(|sink| !sink.empty());
                    let now = Instant::now();
                    if may_close(now, last_play, voices.len()) {
                        output = None;
                    }
                    let Some(message) = message else { continue };
                    if matches!(&message,Message::Play(_,volume) if *volume<=0.0) {
                        continue;
                    }
                    last_play = now;
                    if output.is_none() {
                        if !may_open(now, failed) {
                            continue;
                        }
                        match OutputStream::try_default() {
                            Ok(stream) => {
                                output = Some(stream);
                                failed = None;
                            }
                            Err(error) => {
                                crate::trace::event(crate::trace::Event::AudioInitFailure);
                                failed = Some(now);
                                eprintln!("Audio unavailable; retrying on a later play: {error}");
                                continue;
                            }
                        }
                    }
                    let Message::Play(path, volume) = message else {
                        continue;
                    };
                    if voices.len() >= 8 {
                        continue;
                    }
                    let Some(bytes) = assets::get(&path) else {
                        continue;
                    };
                    let Ok(source) = Decoder::new(Cursor::new(bytes.into_owned())) else {
                        continue;
                    };
                    let handle = &output.as_ref().unwrap().1;
                    match Sink::try_new(handle) {
                        Ok(sink) => {
                            sink.set_volume(volume.clamp(0.0, 1.0));
                            sink.append(source);
                            voices.push(sink);
                        }
                        Err(_) => {
                            crate::trace::event(crate::trace::Event::AudioInitFailure);
                            voices.clear();
                            output = None;
                            failed = Some(now);
                        }
                    }
                }
                drop(voices);
                drop(output);
            })
            .ok();
        Self { sender }
    }
    pub fn warm(&self) {
        let _ = self.sender.try_send(Message::Warm);
    }
    pub fn play(&self, path: impl Into<String>, volume: f32) {
        if volume > 0.0 {
            let _ = self.sender.try_send(Message::Play(path.into(), volume));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_audio_closes_and_failed_devices_can_retry() {
        let now = Instant::now();
        assert!(may_open(now, None));
        assert!(!may_open(now, Some(now)));
        assert!(may_open(now + RETRY, Some(now)));
        assert!(!may_close(now, now, 0));
        assert!(!may_close(now + IDLE, now, 1));
        assert!(may_close(now + IDLE, now, 0));
        // Construction alone sends no warm/play and does not open an audio device.
        drop(Audio::new());
    }
    #[test]
    fn every_bundled_sound_decodes() {
        let names = pingloom_core::PingId::ALL
            .iter()
            .map(|id| id.asset().sound)
            .chain([pingloom_core::model::TICK_SOUND]);
        for name in names {
            assert!(
                assets::get(format!("sounds/{name}.ogg")).is_some(),
                "{name}"
            );
        }
        for dir in ["sounds", "emotes"] {
            for file in assets::files(dir) {
                if file.extension().is_some_and(|ext| ext == "ogg") {
                    assert!(
                        rodio::Decoder::new(std::io::Cursor::new(
                            assets::get(&file).unwrap().into_owned()
                        ))
                        .is_ok(),
                        "{}",
                        file.display()
                    );
                }
            }
        }
    }
}
