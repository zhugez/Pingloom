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
