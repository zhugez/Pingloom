use std::time::{Duration, Instant};

type Generation = u64;

#[derive(Debug, Clone)]
struct Live {
    generation: Generation,
    session: Option<String>,
    last_activity: Instant,
}

#[derive(Debug, Clone)]
enum Phase {
    Closed,
    Opening(Generation),
    Live(Live),
    Closing(Live),
    // Legacy names: these now mean owner-barrier pending/in-flight, not input resume.
    // Reopen waits for accepted saves to finish without changing gesture state.
    ResumePending(Generation),
    Resuming(Generation),
}

#[derive(Debug, Clone)]
pub(crate) struct Lifecycle {
    next_generation: Generation,
    phase: Phase,
    watchdog_running: bool,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            next_generation: 1,
            phase: Phase::Closed,
            watchdog_running: false,
        }
    }
}

impl Lifecycle {
    pub(crate) fn claim_watchdog(&mut self) -> bool {
        if self.watchdog_running || matches!(self.phase, Phase::Closed) {
            return false;
        }
        self.watchdog_running = true;
        true
    }

    // Admission and retirement share the lifecycle lock: a racing open either
    // retains this worker or starts a new one, never losing its watchdog.
    pub(crate) fn keep_watchdog(&mut self) -> bool {
        if matches!(self.phase, Phase::Closed) {
            self.watchdog_running = false;
        }
        self.watchdog_running
    }

    pub(crate) fn watchdog_spawn_failed(&mut self, generation: u64) {
        self.watchdog_running = false;
        self.opening_failed_before_enqueue(generation);
    }

    pub(crate) fn reserve_open(&mut self) -> Option<u64> {
        if !matches!(self.phase, Phase::Closed) {
            return None;
        }
        let generation = self.next_generation;
        self.next_generation = generation.checked_add(1)?;
        self.phase = Phase::Opening(generation);
        Some(generation)
    }

    pub(crate) fn opening_failed_before_enqueue(&mut self, generation: u64) {
        if matches!(self.phase, Phase::Opening(g) if g == generation) {
            self.phase = Phase::Closed;
        }
    }

    pub(crate) fn is_opening(&self, generation: u64) -> bool {
        matches!(self.phase, Phase::Opening(g) if g == generation)
    }

    pub(crate) fn opened(&mut self, generation: u64, now: Instant) -> bool {
        if !matches!(self.phase, Phase::Opening(g) if g == generation) {
            return false;
        }
        self.phase = Phase::Live(Live {
            generation,
            session: None,
            last_activity: now,
        });
        true
    }

    pub(crate) fn need_resume(&mut self, generation: u64) {
        if matches!(self.phase, Phase::Opening(g) if g == generation) {
            self.phase = Phase::ResumePending(generation);
        }
    }

    pub(crate) fn bind_session(&mut self, generation: u64, session: String, now: Instant) -> bool {
        match &mut self.phase {
            Phase::Live(live) if live.generation == generation => {
                live.session = Some(session);
                live.last_activity = now;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn active_generation(&self) -> Option<u64> {
        match &self.phase {
            Phase::Live(live) => Some(live.generation),
            _ => None,
        }
    }

    pub(crate) fn accept_session(&self, session: &str) -> Option<u64> {
        match &self.phase {
            Phase::Live(live) if live.session.as_deref() == Some(session) => Some(live.generation),
            _ => None,
        }
    }

    pub(crate) fn heartbeat(&mut self, session: &str, now: Instant) -> bool {
        match &mut self.phase {
            Phase::Live(live) if live.session.as_deref() == Some(session) => {
                live.last_activity = now;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn claim_close(
        &mut self,
        generation: u64,
        now: Instant,
        expired_only: bool,
    ) -> bool {
        let is_live = matches!(&self.phase, Phase::Live(live) if live.generation == generation && (!expired_only || now.saturating_duration_since(live.last_activity) >= Duration::from_secs(30)));
        if is_live {
            let phase = std::mem::replace(&mut self.phase, Phase::Closed);
            if let Phase::Live(live) = phase {
                self.phase = Phase::Closing(live);
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn finish_close(&mut self, generation: u64, destroyed: bool, now: Instant) {
        let phase = std::mem::replace(&mut self.phase, Phase::Closed);
        match phase {
            Phase::Closing(mut live) if live.generation == generation => {
                if destroyed {
                    self.phase = Phase::ResumePending(generation);
                } else {
                    live.last_activity = now;
                    self.phase = Phase::Live(live);
                }
            }
            other => self.phase = other,
        }
    }

    pub(crate) fn destroyed(&mut self, generation: u64) -> bool {
        if matches!(&self.phase, Phase::Live(live) if live.generation == generation) {
            self.phase = Phase::ResumePending(generation);
            true
        } else {
            false
        }
    }

    pub(crate) fn claim_resume(&mut self) -> Option<u64> {
        let Phase::ResumePending(generation) = self.phase else {
            return None;
        };
        self.phase = Phase::Resuming(generation);
        Some(generation)
    }

    pub(crate) fn finish_resume(&mut self, generation: u64, acknowledged: bool) {
        if matches!(self.phase, Phase::Resuming(g) if g == generation) {
            self.phase = if acknowledged {
                Phase::Closed
            } else {
                Phase::ResumePending(generation)
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(seconds: u64) -> Instant {
        Instant::now() + Duration::from_secs(seconds)
    }
    fn live() -> (Lifecycle, u64) {
        let mut state = Lifecycle::default();
        let generation = state.reserve_open().unwrap();
        assert!(state.opened(generation, at(0)));
        assert!(state.bind_session(generation, "s".into(), at(0)));
        (state, generation)
    }

    #[test]
    fn heartbeat_wins_expiry_and_post_claim_calls_reject() {
        let (mut s, g) = live();
        assert!(s.heartbeat("s", at(29)));
        assert!(!s.claim_close(g, at(30), true));
        assert!(s.claim_close(g, at(60), true));
        assert!(!s.heartbeat("s", at(61)));
        assert_eq!(s.active_generation(), None);
        assert_eq!(s.accept_session("s"), None);
        assert!(!s.bind_session(g, "late".into(), at(61)));
    }

    #[test]
    fn destroy_failure_restores_session_and_rearms_deadline() {
        let (mut s, g) = live();
        assert!(s.claim_close(g, at(31), true));
        s.finish_close(g, false, at(31));
        assert_eq!(s.accept_session("s"), Some(g));
        assert!(!s.claim_close(g, at(60), true));
        assert!(s.claim_close(g, at(62), true));
    }

    #[test]
    fn pending_resume_retries_and_only_ack_opens_next_generation() {
        let (mut s, g) = live();
        assert!(s.destroyed(g));
        assert!(!s.destroyed(g));
        assert_eq!(s.active_generation(), None);
        assert_eq!(s.reserve_open(), None);
        assert_eq!(s.claim_resume(), Some(g));
        assert_eq!(s.claim_resume(), None);
        s.finish_resume(g, false);
        assert_eq!(s.claim_resume(), Some(g));
        s.finish_resume(g, true);
        let next = s.reserve_open().unwrap();
        assert!(next > g);
        assert!(s.opened(next, at(0)));
        s.destroyed(g);
        s.finish_resume(g, true);
        assert_eq!(s.active_generation(), Some(next));
    }

    #[test]
    fn opening_pre_enqueue_failure_closes_but_accepted_failure_needs_resume() {
        let mut s = Lifecycle::default();
        let g = s.reserve_open().unwrap();
        assert!(s.is_opening(g));
        assert_eq!(s.reserve_open(), None);
        s.opening_failed_before_enqueue(g);
        assert!(!s.is_opening(g));
        let next = s.reserve_open().unwrap();
        assert!(next > g);
        s.need_resume(next);
        assert_eq!(s.reserve_open(), None);
        assert_eq!(s.claim_resume(), Some(next));
        s.finish_resume(next, true);
        assert!(s.reserve_open().is_some());
    }

    #[test]
    fn stale_and_duplicate_completions_do_not_mutate_replacement() {
        let (mut s, g) = live();
        assert!(s.claim_close(g, at(1), false));
        s.finish_close(g, true, at(1));
        let claimed = s.claim_resume().unwrap();
        s.finish_resume(claimed, true);
        let replacement = s.reserve_open().unwrap();
        assert!(s.opened(replacement, at(0)));
        s.finish_close(g, false, at(2));
        s.finish_resume(g, true);
        s.opening_failed_before_enqueue(g);
        s.need_resume(g);
        assert_eq!(s.active_generation(), Some(replacement));
    }

    #[test]
    fn close_requires_matching_generation_and_non_expiry_close_is_idempotent() {
        let (mut s, g) = live();
        assert!(!s.claim_close(g + 1, at(0), false));
        assert!(s.claim_close(g, at(0), false));
        assert!(!s.claim_close(g, at(0), false));
        s.finish_close(g, true, at(0));
        assert_eq!(s.claim_resume(), Some(g));
    }

    #[test]
    fn scheduled_close_failure_leaves_live_until_main_thread_claim() {
        let (mut s, g) = live();
        // A failed run_on_main_thread submission performs no claim.
        assert_eq!(s.accept_session("s"), Some(g));
        assert_eq!(s.active_generation(), Some(g));
        // The accepted main-thread closure claims once; a competing callback is inert.
        assert!(s.claim_close(g, at(1), false));
        assert!(!s.claim_close(g, at(1), false));
        s.finish_close(g, true, at(1));
        assert_eq!(s.claim_resume(), Some(g));
    }

    #[test]
    fn destroyed_callback_only_recovers_live_matching_generation() {
        let (mut s, g) = live();
        assert!(s.destroyed(g));
        assert!(!s.destroyed(g));
        assert_eq!(s.claim_resume(), Some(g));
        s.finish_resume(g, true);
        let replacement = s.reserve_open().unwrap();
        assert!(s.opened(replacement, at(0)));
        assert!(!s.destroyed(g));
        assert_eq!(s.active_generation(), Some(replacement));
    }

    #[test]
    fn save_session_admission_rejects_after_close_claim() {
        let (mut s, g) = live();
        assert!(s.claim_close(g, at(1), false));
        assert_eq!(s.accept_session("s"), None);
    }

    #[test]
    fn watchdog_retires_only_when_closed_and_rearms_without_duplicate_workers() {
        let mut s = Lifecycle::default();
        assert!(!s.claim_watchdog());
        let g = s.reserve_open().unwrap();
        assert!(s.claim_watchdog());
        assert!(!s.claim_watchdog());
        assert!(s.keep_watchdog(), "opening still needs recovery");
        assert!(s.opened(g, at(0)));
        assert!(s.keep_watchdog());
        assert!(s.claim_close(g, at(1), false));
        assert!(s.keep_watchdog(), "close callback is still pending");
        s.finish_close(g, true, at(1));
        assert!(s.keep_watchdog());
        assert_eq!(s.claim_resume(), Some(g));
        assert!(s.keep_watchdog(), "barrier completion is still pending");
        s.finish_resume(g, false);
        assert!(s.keep_watchdog(), "failed barrier must be retried");
        assert_eq!(s.claim_resume(), Some(g));
        s.finish_resume(g, true);
        // Reopen before retirement retains the existing worker.
        let next = s.reserve_open().unwrap();
        assert!(!s.claim_watchdog());
        assert!(s.keep_watchdog());
        s.opening_failed_before_enqueue(next);
        assert!(!s.keep_watchdog());
        // Reopen after retirement admits exactly one new worker.
        let next = s.reserve_open().unwrap();
        assert!(s.claim_watchdog());
        assert!(!s.claim_watchdog());
        s.watchdog_spawn_failed(next);
        assert!(!s.keep_watchdog());
        s.reserve_open().unwrap();
        assert!(s.claim_watchdog(), "spawn failure must permit retry");
    }

    #[test]
    fn generations_reject_exhaustion() {
        let mut s = Lifecycle {
            next_generation: u64::MAX,
            phase: Phase::Closed,
            watchdog_running: false,
        };
        assert_eq!(s.reserve_open(), None);
    }
}
