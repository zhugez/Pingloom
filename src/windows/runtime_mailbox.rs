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
    pub settings_suspended: bool,
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
    Suspend(bool, mpsc::Sender<Result<RuntimeStatus, OwnerError>>),
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
        let mailbox = Self::with_wake(|_| true);
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
    pub(crate) fn suspend(self: &Arc<Self>, value: bool) -> Result<StatusReply, OwnerError> {
        let (tx, rx) = mpsc::channel();
        self.enqueue(OwnerRequest::Suspend(value, tx))?;
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
        OwnerRequest::Suspend(_, tx) | OwnerRequest::Save(_, tx) => {
            let _ = tx.send(Err(error()));
        }
        OwnerRequest::Quit(tx) => {
            let _ = tx.send(Err(error()));
        }
    }
}

pub(crate) fn wait_reply<T>(rx: mpsc::Receiver<Result<T, OwnerError>>) -> Result<T, OwnerError> {
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| OwnerError::unavailable("native owner reply timed out"))?
}

// Accepted suspension must retain its reply until the owner confirms the transition.
pub(crate) fn wait_suspend_reply<T>(
    rx: mpsc::Receiver<Result<T, OwnerError>>,
) -> Result<T, OwnerError> {
    rx.recv().map_err(|_| {
        OwnerError::unavailable("native owner disconnected before confirming suspension")
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
