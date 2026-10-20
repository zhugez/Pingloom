#!/usr/bin/env python3
"""Apply the reviewed integration to the pinned niri 26.04 source, never a live install."""
from pathlib import Path
import shutil
import sys
import tomllib

root, project = map(Path, sys.argv[1:])
assert tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"] == "26.4.0"


def replace(file, old, new):
    path = root / file
    source = path.read_text()
    assert source.count(old) == 1, (file, old[:100], source.count(old))
    path.write_text(source.replace(old, new))


replace("src/lib.rs", "pub mod protocols;", "pub mod protocols;\npub mod pingloom;")
replace("src/niri.rs", "pub struct Niri {", "pub struct Niri {\n    pub pingloom: crate::pingloom::Bridge,")
replace("src/niri.rs", "        let mut niri = Self {", "        let mut niri = Self {\n            pingloom: crate::pingloom::Bridge::default(),")
replace("src/niri.rs", "    fn refresh(&mut self) {", "    fn refresh(&mut self) {\n        self.pingloom_refresh();")
replace("src/niri.rs", "        let mut state = Self { backend, niri };", """        let mut state = Self { backend, niri };
        state.niri.event_loop.insert_source(
            Timer::from_duration(Duration::from_millis(100)),
            |_, _, state| {
                state.pingloom_refresh();
                TimeoutAction::ToDuration(Duration::from_millis(100))
            },
        ).unwrap();""")
replace("src/backend/tty.rs", "    pub fn set_monitors_active(&mut self, active: bool) {", "    pub fn pingloom_session_active(&self) -> bool { self.session.is_active() }\n\n    pub fn set_monitors_active(&mut self, active: bool) {")
replace("niri-ipc/src/lib.rs", "pub enum Request {", "pub enum Request {\n    /// Private, versioned Pingloom compositor integration.\n    Pingloom(serde_json::Value),")
replace("src/ipc/server.rs", "    let response = match request {", """    let response = match request {
        Request::Pingloom(value) => {
            let (tx, rx) = async_channel::bounded(1);
            ctx.event_loop.insert_idle(move |state| {
                let result = state.pingloom_control(value);
                let _ = tx.try_send(result);
            });
            rx.recv().await.map_err(|e| e.to_string())??;
            Response::Handled
        }""")

path = root / "src/input/mod.rs"
source = path.read_text()
start = source.index("    fn on_keyboard<I: InputBackend>(")
end = source.index("    fn start_key_repeat(", start)
old = source[start:end]
body = old[old.index("        let mod_key"):]
body = body.replace("        let time = Event::time_msec(&event);\n", "")
body = body.replace("event.state()", "key_state").replace("event.key_code()", "key_code")
assert "event." not in body
new = """    fn on_keyboard<I: InputBackend>(&mut self, event: I::KeyboardKeyEvent, consumed_by_a11y: &mut bool) {
        self.pingloom_native_key(event.key_code(), event.state(), Event::time_msec(&event), consumed_by_a11y, false);
    }

    pub(crate) fn pingloom_native_key(&mut self, key_code: Keycode, key_state: KeyState, time: u32, consumed_by_a11y: &mut bool, replaying: bool) {
        if !replaying && self.pingloom_key(key_code, key_state, time) { return; }
""" + body
source = source[:start] + new + source[end:]
start = source.index("    fn on_pointer_button<I: InputBackend>(")
end = source.index("    fn on_pointer_axis<I: InputBackend>(", start)
old = source[start:end]
body = old[old.index("        let pointer"):]
body = body.replace("        let button = event.button();", """        let button = match button_code {
            0x110 => Some(MouseButton::Left), 0x111 => Some(MouseButton::Right),
            0x112 => Some(MouseButton::Middle), 0x113 | 0x116 => Some(MouseButton::Back),
            0x114 | 0x115 => Some(MouseButton::Forward), _ => None,
        };""")
body = body.replace("        let button_code = event.button_code();\n", "").replace("        let button_state = event.state();\n", "")
body = body.replace("event.time_msec()", "time")
assert "event." not in body
new = """    fn on_pointer_button<I: InputBackend>(&mut self, event: I::PointerButtonEvent) {
        self.pingloom_native_button(event.button_code(), event.state(), event.time_msec(), false);
    }

    pub(crate) fn pingloom_native_button(&mut self, button_code: u32, button_state: ButtonState, time: u32, replaying: bool) {
        if !replaying && self.pingloom_button(button_code, button_state, time) { return; }
""" + body
source = source[:start] + new + source[end:]
# These are the two normal relative/absolute motion delivery paths, after constraints/clamping.
for function, following in [("on_pointer_motion<I:", "on_pointer_motion_absolute<I:"), ("on_pointer_motion_absolute<I:", "on_pointer_button<I:")]:
    start = source.index("    fn " + function)
    end = source.index("    fn " + following, start)
    block = source[start:end]
    marker = "        pointer.frame(self);"
    pos = block.rfind(marker)
    assert pos >= 0
    block = block[:pos] + block[pos:].replace(marker, marker + "\n        self.pingloom_motion(event.time_msec());", 1)
    source = source[:start] + block + source[end:]
path.write_text(source)

replace("Cargo.toml", "[dependencies]\n", '[dependencies]\npingloom-core = { path = "pingloom-core" }\n')
core = root / "pingloom-core"
(core / "src").mkdir(parents=True)
(core / "Cargo.toml").write_text('''[package]
name = "pingloom-core"
version = "0.1.0"
edition = "2021"
license = "MIT"
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
''')
for name in ["lib.rs", "model.rs", "gesture.rs", "geometry.rs", "settings.rs", "niri_bridge.rs"]:
    shutil.copyfile(project / "src" / name, core / "src" / name)
(core / "assets").symlink_to(project / "assets", target_is_directory=True)
shutil.copyfile(project / "nix/niri/compositor.rs", root / "src/pingloom.rs")
lock = (root / "Cargo.lock").read_text()
start = lock.index('[[package]]\nname = "niri"\n')
end = lock.index('[[package]]', start + 12)
block = lock[start:end]
assert ' "pingloom-core",' not in block
block = block.replace('dependencies = [\n', 'dependencies = [\n "pingloom-core",\n', 1)
lock = lock[:start] + block + lock[end:]
lock += '''\n[[package]]
name = "pingloom-core"
version = "0.1.0"
dependencies = [
 "serde",
 "serde_json",
 "tempfile",
]
'''
(root / "Cargo.lock").write_text(lock)
print("Applied Pingloom integration to pinned niri source")
