//! Native niri transport: semantic actions from the pinned compositor integration.
//! Never read /dev/input, poll the cursor, steal focus, or replay through XTest.
use super::Shared;
use pingloom_core::{
    niri_bridge::{Control, Operation, Packet, VERSION},
    Settings,
};
use std::{
    fs::{self, DirBuilder, File},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
        net::{UnixDatagram, UnixStream},
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, OnceLock,
    },
    time::{Duration, Instant},
};

static CONTROL: OnceLock<mpsc::SyncSender<Command>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicBool = AtomicBool::new(false);
enum Command {
    Configure(Box<Settings>, mpsc::Sender<Result<(), String>>),
    Enable(bool),
    Stop,
}

pub(super) fn active() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
}
pub(super) fn apply(settings: &Settings) -> Result<(), String> {
    if !LIVE.load(Ordering::Acquire) {
        return Ok(()); // Offline Settings can still be repaired and persisted.
    }
    let Some(tx) = CONTROL.get() else {
        return Ok(());
    }; // Settings remains usable before backend admission.
    let (reply, rx) = mpsc::channel();
    tx.try_send(Command::Configure(Box::new(settings.clone()), reply))
        .map_err(|_| "niri control queue unavailable")?;
    rx.recv_timeout(Duration::from_secs(2))
        .map_err(|_| "niri configuration acknowledgement timed out")?
}
pub(super) fn enabled(value: bool) {
    if ENABLED.swap(value, Ordering::AcqRel) != value {
        if let Some(tx) = CONTROL.get() {
            if tx.try_send(Command::Enable(value)).is_err() {
                FAILED.store(true, Ordering::Release);
            }
        }
    }
}
pub(super) fn received_enabled(value: bool) {
    ENABLED.store(value, Ordering::Release);
}
pub(super) fn stop() {
    if let Some(tx) = CONTROL.get() {
        let _ = tx.try_send(Command::Stop);
    }
}

fn request(control: &Control) -> Result<(), String> {
    let path = std::env::var_os("NIRI_SOCKET").ok_or(
        "Native Wayland gestures require the Pingloom-integrated niri build (NIRI_SOCKET missing)",
    )?;
    let mut stream =
        UnixStream::connect(path).map_err(|e| format!("niri bridge unavailable: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(750)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_millis(750)))
        .map_err(|e| e.to_string())?;
    let mut bytes =
        serde_json::to_vec(&serde_json::json!({"Pingloom":control})).map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Compositor command exceeds size limit".into());
    }
    bytes.push(b'\n');
    stream.write_all(&bytes).map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    BufReader::new(stream)
        .take(65537)
        .read_until(b'\n', &mut response)
        .map_err(|e| e.to_string())?;
    if response.len() > 65536 {
        return Err("Compositor response exceeds size limit".into());
    }
    let reply: serde_json::Value = serde_json::from_slice(&response).map_err(|e| e.to_string())?;
    if reply.get("Ok") == Some(&serde_json::json!("Handled")) {
        Ok(())
    } else {
        let is_stock = reply
            .get("Err")
            .and_then(|e| e.as_str())
            .is_some_and(|s| s.contains("error parsing request"));
        if is_stock {
            Err("Detected stock niri compositor (unpatched IPC). Settings mode remains active; in-compositor gestures require the pinned Pingloom-integrated niri package.".into())
        } else {
            Err(format!("niri rejected the bridge command: {reply}. Use the pinned Pingloom-integrated niri package; stock niri is not supported."))
        }
    }
}
struct SocketPath(PathBuf);
impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn event_socket() -> Result<(UnixDatagram, SocketPath), String> {
    let runtime =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR missing")?);
    if !runtime.is_absolute() {
        return Err("Runtime directory must be absolute".into());
    }
    let owner = fs::metadata(&runtime).map_err(|e| e.to_string())?.uid();
    let directory = runtime.join("pingloom");
    match DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let metadata = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err("Unsafe private Pingloom runtime directory".into());
    }
    let path = directory.join("niri-events.sock");
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.file_type().is_socket() || meta.uid() != owner {
            return Err("Unsafe existing bridge socket".into());
        }
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let socket = UnixDatagram::bind(&path).map_err(|e| e.to_string())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(20)))
        .map_err(|e| e.to_string())?;
    Ok((socket, SocketPath(path)))
}
fn session_id() -> Result<String, String> {
    let mut random = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut random))
        .map_err(|e| e.to_string())?;
    Ok(random.iter().map(|b| format!("{b:02x}")).collect())
}
fn command(session: &str, operation: Operation) -> Control {
    Control {
        version: VERSION,
        session: session.into(),
        operation,
    }
}

pub(super) fn run(shared: Arc<Shared>) -> Result<(), String> {
    let (socket, path) = event_socket()?;
    let session = session_id()?;
    // Serialize admission with the complete Settings save transaction, not the
    // input mutex. No native interception is enabled while an offline save runs.
    let admission = shared
        .configuration
        .lock()
        .map_err(|_| "Settings owner unavailable")?;
    let settings = shared
        .state
        .lock()
        .map_err(|_| "Owner unavailable")?
        .engine
        .settings()
        .clone();
    request(&command(
        &session,
        Operation::Connect {
            enabled: false,
            settings: settings.clone(),
            socket: path.0.to_string_lossy().into_owned(),
        },
    ))?;
    let (tx, rx) = mpsc::sync_channel(16);
    CONTROL
        .set(tx)
        .map_err(|_| "Compositor transport already initialized")?;
    LIVE.store(true, Ordering::Release);
    let admitted = request(&command(
        &session,
        Operation::Enable(settings.enabled_on_start),
    ))
    .map(|()| shared.ready());
    drop(admission);
    let result = admitted.and_then(|()| {
        eprintln!("Native niri gesture bridge ready");
        receive(&shared, &socket, &session, rx)
    });
    LIVE.store(false, Ordering::Release);
    let _ = request(&command(&session, Operation::Disconnect));
    shared.overflow.store(true, Ordering::Release);
    super::super::wake_owner();
    result
}
fn receive(
    shared: &Shared,
    socket: &UnixDatagram,
    session: &str,
    rx: mpsc::Receiver<Command>,
) -> Result<(), String> {
    let mut heartbeat = Instant::now();
    let mut last_packet = Instant::now();
    let mut sequence = 0;
    let mut bytes = [0u8; 16385];
    while shared.running.load(Ordering::Acquire) {
        if shared
            .owner_progress
            .lock()
            .map_err(|_| "Owner unavailable")?
            .elapsed()
            > Duration::from_millis(pingloom_core::niri_bridge::LEASE_MS)
        {
            return Err("Native renderer/owner stopped responding; gestures paused".into());
        }
        if FAILED.load(Ordering::Acquire) {
            return Err("Compositor control queue unavailable; gestures paused".into());
        }
        while let Ok(c) = rx.try_recv() {
            match c {
                Command::Configure(settings, reply) => {
                    let result = request(&command(session, Operation::Configure(*settings)));
                    let failed = result.is_err();
                    let _ = reply.send(result);
                    if failed {
                        return Err("niri configuration failed; gestures paused".into());
                    }
                }
                Command::Enable(enabled) => request(&command(session, Operation::Enable(enabled)))?,
                Command::Stop => return Ok(()),
            }
        }
        if heartbeat.elapsed() >= Duration::from_millis(400) {
            request(&command(session, Operation::Heartbeat))?;
            heartbeat = Instant::now();
        }
        match socket.recv(&mut bytes) {
            Ok(length) => {
                if length > 16384 {
                    return Err("Oversized niri event packet".into());
                }
                let packet: Packet =
                    serde_json::from_slice(&bytes[..length]).map_err(|e| e.to_string())?;
                if packet.version != VERSION
                    || packet.session != session
                    || packet.sequence <= sequence
                {
                    continue;
                }
                if !packet.scale.is_finite()
                    || !(0.25..=8.0).contains(&packet.scale)
                    || packet.actions.len() > 64
                {
                    return Err("Invalid compositor scale/action bounds".into());
                }
                if sequence != 0 && packet.sequence != sequence + 1 {
                    return Err("Compositor event sequence gap; gestures paused".into());
                }
                sequence = packet.sequence;
                last_packet = Instant::now();
                shared.niri_packet(packet)?;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e.to_string()),
        }
        if last_packet.elapsed() > Duration::from_millis(pingloom_core::niri_bridge::LEASE_MS) {
            return Err("Compositor bridge stopped responding; gestures paused".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn bridge_control_is_versioned_not_an_arbitrary_compositor_action() {
        let control = super::command(
            "0123456789abcdef0123456789abcdef",
            pingloom_core::niri_bridge::Operation::Enable(false),
        );
        let value = serde_json::to_value(control).unwrap();
        assert_eq!(value["version"], 1);
        assert!(value.get("Action").is_none());
    }
}
