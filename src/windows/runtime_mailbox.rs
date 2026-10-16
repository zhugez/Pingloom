use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use pingloom_core::settings::Settings;
#[cfg(windows)]
use windows_sys::Win32::Foundation::HWND;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

#[cfg(windows)]
pub(crate) const WM_OWNER_REQUEST: u32 = 0x8000 + 18;
const CAPACITY: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeStatus {
    pub enabled: bool,
    pub load_warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OwnerError {
    pub code: &'static str,
    pub message: String,
    pub field: Option<String>,
}

impl OwnerError {
    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "unavailable",
            message: message.into(),
            field: None,
        }
    }
}

pub(crate) type QueryResult = (Settings, RuntimeStatus);
pub(crate) type QueryReply = mpsc::Receiver<Result<QueryResult, OwnerError>>;
pub(crate) type StatusReply = mpsc::Receiver<Result<RuntimeStatus, OwnerError>>;

pub(crate) enum OwnerRequest {
    Query(mpsc::Sender<Result<QueryResult, OwnerError>>),
    Status(mpsc::Sender<Result<RuntimeStatus, OwnerError>>),
    // Orders Settings lifecycle transitions after accepted saves without touching input.
    Barrier(mpsc::Sender<Result<(), OwnerError>>),
    Save(
        Box<Settings>,
        mpsc::Sender<Result<RuntimeStatus, OwnerError>>,
    ),
    Quit(mpsc::Sender<Result<(), OwnerError>>),
}

struct State {
    // Stored as an integer because HWND is thread-affine; only the owner publishes/uses it.
    hwnd: isize,
    closing: bool,
    queue: VecDeque<OwnerRequest>,
    wake: fn(isize) -> bool,
}

pub(crate) struct OwnerMailbox {
    state: Mutex<State>,
}

impl OwnerMailbox {
    pub(crate) fn new() -> Arc<Self> {
        #[cfg(windows)]
        let mailbox = Self::with_wake(|hwnd| unsafe {
            PostMessageW(hwnd as HWND, WM_OWNER_REQUEST, 0, 0) != 0
        });
        #[cfg(not(windows))]
        let mailbox = Self::with_wake(|_| crate::desktop::wake_owner());
        mailbox
    }

    fn with_wake(wake: fn(isize) -> bool) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                hwnd: 0,
                closing: false,
                queue: VecDeque::new(),
                wake,
            }),
        })
    }

    #[cfg(windows)]
    pub(crate) fn publish_hwnd(&self, hwnd: HWND) {
        self.publish_owner(hwnd as isize);
    }

    pub(crate) fn publish_owner(&self, identity: isize) {
        self.state.lock().expect("owner mailbox poisoned").hwnd = identity;
    }

    pub(crate) fn enqueue(&self, request: OwnerRequest) -> Result<(), OwnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| OwnerError::unavailable("owner mailbox unavailable"))?;
        if state.closing {
            return Err(OwnerError::unavailable("native owner is shutting down"));
        }
        if state.queue.len() >= CAPACITY {
            return Err(OwnerError {
                code: "busy",
                message: "native owner queue is full".into(),
                field: None,
            });
        }
        if state.hwnd == 0 {
            return Err(OwnerError::unavailable("native owner is not ready"));
        }
        state.queue.push_back(request);
        // Keep the queue lock through wake so a failed PostMessage can safely remove this request.
        if !(state.wake)(state.hwnd) {
            let request = state.queue.pop_back().expect("just-enqueued owner request");
            reject_request(request, "could not wake native owner");
            return Err(OwnerError::unavailable("could not wake native owner"));
        }
        Ok(())
    }

    pub(crate) fn take_all(&self) -> Vec<OwnerRequest> {
        let mut state = self.state.lock().expect("owner mailbox poisoned");
        state.queue.drain(..).collect()
    }

    pub(crate) fn close_and_reject_pending(&self) {
        let pending = {
            let mut state = self.state.lock().expect("owner mailbox poisoned");
            state.closing = true;
            state.queue.drain(..).collect::<Vec<_>>()
        };
        for request in pending {
            reject_request(request, "native owner stopped before processing request");
        }
    }

    pub(crate) fn query(self: &Arc<Self>) -> Result<QueryReply, OwnerError> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(OwnerRequest::Query(tx))?;
        Ok(rx)
    }
    pub(crate) fn status(self: &Arc<Self>) -> Result<StatusReply, OwnerError> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(OwnerRequest::Status(tx))?;
        Ok(rx)
    }
    pub(crate) fn barrier(
        self: &Arc<Self>,
    ) -> Result<mpsc::Receiver<Result<(), OwnerError>>, OwnerError> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(OwnerRequest::Barrier(tx))?;
        Ok(rx)
    }
    pub(crate) fn save(self: &Arc<Self>, settings: Settings) -> Result<StatusReply, OwnerError> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(OwnerRequest::Save(Box::new(settings), tx))?;
        Ok(rx)
    }
    pub(crate) fn quit(
        self: &Arc<Self>,
    ) -> Result<mpsc::Receiver<Result<(), OwnerError>>, OwnerError> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(OwnerRequest::Quit(tx))?;
        Ok(rx)
    }
}

fn reject_request(request: OwnerRequest, message: &str) {
    let error = || OwnerError::unavailable(message);
    match request {
        OwnerRequest::Query(tx) => {
            let _ = tx.send(Err(error()));
        }
        OwnerRequest::Status(tx) | OwnerRequest::Save(_, tx) => {
            let _ = tx.send(Err(error()));
        }
        OwnerRequest::Barrier(tx) | OwnerRequest::Quit(tx) => {
            let _ = tx.send(Err(error()));
        }
    }
}

pub(crate) fn wait_reply<T>(rx: mpsc::Receiver<Result<T, OwnerError>>) -> Result<T, OwnerError> {
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| OwnerError::unavailable("native owner reply timed out"))?
}

// Keep lifecycle admission ordered after all previously accepted owner requests.
pub(crate) fn wait_barrier_reply(
    rx: mpsc::Receiver<Result<(), OwnerError>>,
) -> Result<(), OwnerError> {
    rx.recv().map_err(|_| {
        OwnerError::unavailable("native owner disconnected before confirming Settings barrier")
    })?
}

// Save completion must be definitive: a timeout could report failure before a queued save commits.
pub(crate) fn wait_save_reply<T>(
    rx: mpsc::Receiver<Result<T, OwnerError>>,
) -> Result<T, OwnerError> {
    rx.recv().map_err(|_| OwnerError {
        code: "indeterminate",
        message: "Save outcome is indeterminate: native owner disconnected before confirming it"
            .into(),
        field: None,
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_wake(_: isize) -> bool {
        true
    }

    #[test]
    fn lifecycle_barriers_preserve_manual_pause_and_order_accepted_saves() {
        use crate::settings_lifecycle::Lifecycle;
        use pingloom_core::GestureEngine;
        let mailbox = OwnerMailbox::with_wake(fake_wake);
        mailbox.publish_owner(1);
        let mut engine = GestureEngine::new(Settings::default()).unwrap();
        for enabled in [true, false] {
            engine.set_enabled(enabled);
            let mut lifecycle = Lifecycle::default();
            let generation = lifecycle.reserve_open().unwrap();
            // An already-held gesture must also survive opening Settings.
            use pingloom_core::{Button, InputEvent, Point};
            engine.on_event(InputEvent::KeyDown {
                vk: 0x12,
                injected: false,
            });
            engine.on_event(InputEvent::PointerDown {
                button: Button::Left,
                point: Point::new(100, 100),
                time_ms: 100,
                injected: false,
                fullscreen_app: false,
                dpi_scale: 1.0,
            });
            engine.on_event(InputEvent::PointerMove {
                point: Point::new(200, 100),
                time_ms: 110,
                injected: false,
            });
            let gesture_before_open = engine.state();
            let opened = mailbox.barrier().unwrap();
            let OwnerRequest::Barrier(reply) = mailbox.take_all().pop().unwrap() else {
                panic!("open mutated input")
            };
            reply.send(Ok(())).unwrap();
            wait_barrier_reply(opened).unwrap();
            assert_eq!(engine.state(), gesture_before_open);
            assert_eq!(engine.enabled(), enabled);
            let now = std::time::Instant::now();
            assert!(lifecycle.opened(generation, now));
            assert!(lifecycle.bind_session(generation, "session".into(), now));
            let mut settings = engine.settings().clone();
            settings.volume = 37;
            let saved = mailbox.save(settings).unwrap();
            assert!(lifecycle.claim_close(generation, now, false));
            lifecycle.finish_close(generation, true, now);
            let draining = lifecycle.claim_resume().unwrap();
            let closed = mailbox.barrier().unwrap();
            assert!(lifecycle.reserve_open().is_none());
            assert!(closed.try_recv().is_err());
            for request in mailbox.take_all() {
                match request {
                    OwnerRequest::Save(settings, reply) => {
                        engine.set_settings(*settings).unwrap();
                        reply
                            .send(Ok(RuntimeStatus {
                                enabled: engine.enabled(),
                                load_warning: None,
                            }))
                            .unwrap();
                    }
                    OwnerRequest::Barrier(reply) => {
                        reply.send(Ok(())).unwrap();
                    }
                    _ => panic!("unexpected lifecycle request"),
                }
            }
            assert_eq!(wait_save_reply(saved).unwrap().enabled, enabled);
            wait_barrier_reply(closed).unwrap();
            lifecycle.finish_resume(draining, true);
            assert_eq!(engine.enabled(), enabled);
            assert_eq!(engine.settings().volume, 37);
            assert!(lifecycle.reserve_open().is_some());
        }
    }

    #[test]
    fn save_waits_for_delayed_definitive_completion() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            tx.send(Ok(42)).unwrap();
        });
        assert_eq!(wait_save_reply(rx).unwrap(), 42);
    }

    #[test]
    fn not_ready_rejects_before_queueing() {
        let mailbox = OwnerMailbox::with_wake(fake_wake);
        assert_eq!(mailbox.query().unwrap_err().code, "unavailable");
        assert!(mailbox.state.lock().unwrap().queue.is_empty());
    }

    #[test]
    fn shutdown_rejects_each_queued_reply() {
        let mailbox = OwnerMailbox::with_wake(fake_wake);
        mailbox.publish_owner(1);
        let reply = mailbox.query().unwrap();
        mailbox.close_and_reject_pending();
        assert_eq!(reply.recv().unwrap().unwrap_err().code, "unavailable");
        assert!(mailbox
            .query()
            .unwrap_err()
            .message
            .contains("shutting down"));
    }

    #[test]
    fn queue_limit_returns_busy_without_losing_queued_work() {
        let mailbox = OwnerMailbox::with_wake(fake_wake);
        mailbox.publish_owner(1);
        let replies: Vec<_> = (0..CAPACITY).map(|_| mailbox.query().unwrap()).collect();
        assert_eq!(mailbox.query().unwrap_err().code, "busy");
        assert_eq!(mailbox.take_all().len(), CAPACITY);
        drop(replies);
    }
}
