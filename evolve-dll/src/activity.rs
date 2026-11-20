//! Keeps one core busy while the audio callback keeps arriving. With a low
//! minimum processor state the plugins' block time sometimes stretches 3-4x
//! (past the deadline for the heaviest ones) when every core idles between
//! blocks; one spinning thread is enough to prevent it.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Spinning stops this long after the last pulse.
const IDLE_MS: u64 = 100;

struct Shared {
    stop: AtomicBool,
    /// Milliseconds since `origin` plus one at the last pulse; 0 = never.
    last: AtomicU64,
    spinning: AtomicBool,
    origin: Instant,
}

impl Shared {
    fn now(&self) -> u64 {
        1 + self.origin.elapsed().as_millis() as u64
    }
}

pub struct Activity {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

fn recent(last: u64, now: u64) -> bool {
    last != 0 && now.saturating_sub(last) < IDLE_MS
}

impl Activity {
    pub fn new() -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            last: AtomicU64::new(0),
            spinning: AtomicBool::new(false),
            origin: Instant::now(),
        });
        let state = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("evolve-activity".into())
            .spawn(move || {
                while !state.stop.load(Ordering::Acquire) {
                    let active = recent(state.last.load(Ordering::Acquire), state.now());
                    state.spinning.store(active, Ordering::Release);
                    if active {
                        for _ in 0..256 {
                            std::hint::spin_loop();
                        }
                    } else {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
                state.spinning.store(false, Ordering::Release);
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// Called from the audio callback: an atomic store only.
    pub fn pulse(&self) {
        self.shared.last.store(self.shared.now(), Ordering::Release);
    }

    pub fn spinning(&self) -> bool {
        self.shared.spinning.load(Ordering::Acquire)
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_count::allocations;

    #[test]
    fn an_unstarted_stream_is_idle_and_expiry_is_exclusive() {
        assert!(!recent(0, 1));
        assert!(recent(10, 109));
        assert!(!recent(10, 110));
    }

    #[test]
    fn spins_after_a_pulse_stops_when_pulses_stop_and_joins_on_drop() {
        let activity = Activity::new().unwrap();
        assert!(!activity.spinning());
        let before = allocations();
        activity.pulse();
        assert_eq!(allocations(), before);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !activity.spinning() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        while activity.spinning() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        drop(activity);
    }
}
