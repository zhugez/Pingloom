use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::mpsc::{self, Sender};
use std::thread;

use super::assets::ASSETS;

pub struct Audio {
    sender: Sender<(String, f32)>,
}

impl Audio {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<(String, f32)>();
        thread::Builder::new()
            .name("pingloom-audio".into())
            .spawn(move || {
                let Ok((_stream, handle)) = OutputStream::try_default() else {
                    return;
                };
                let mut voices: Vec<Sink> = Vec::new();
                while let Ok((path, volume)) = receiver.recv() {
                    voices.retain(|sink| !sink.empty());
                    if voices.len() >= 8 || volume <= 0.0 {
                        continue;
                    }
                    let Some(bytes) = ASSETS.get_file(&path).map(|file| file.contents()) else {
                        continue;
                    };
                    let Ok(source) = Decoder::new(Cursor::new(bytes)) else {
                        continue;
                    };
                    let Ok(sink) = Sink::try_new(&handle) else {
                        continue;
                    };
                    sink.set_volume(volume.clamp(0.0, 1.0));
                    sink.append(source);
                    voices.push(sink);
                }
                drop(voices);
            })
            .ok();
        Self { sender }
    }

    pub fn play(&self, path: impl Into<String>, volume: f32) {
        let _ = self.sender.send((path.into(), volume));
    }
}

#[cfg(test)]
mod tests {
    use super::ASSETS;

    #[test]
    fn every_bundled_sound_decodes() {
        let names = pingloom_core::PingId::ALL
            .iter()
            .map(|id| id.asset().sound)
            .chain([pingloom_core::model::TICK_SOUND]);
        for name in names {
            assert!(
                ASSETS.get_file(format!("sounds/{name}.ogg")).is_some(),
                "{name}"
            );
        }
        for dir in ["sounds", "emotes"] {
            for file in ASSETS.get_dir(dir).unwrap().files() {
                if file.path().extension().is_some_and(|ext| ext == "ogg") {
                    assert!(
                        rodio::Decoder::new(std::io::Cursor::new(file.contents())).is_ok(),
                        "{}",
                        file.path().display()
                    );
                }
            }
        }
    }
}
