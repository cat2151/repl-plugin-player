//! Keep the render workload out of the slow mostly-idle CPU regime observed
//! with the reference player's DSP. A missing callback heartbeat stops spinning.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const IDLE_MS: u64 = 100;
struct Shared {
    stop: AtomicBool,
    last: AtomicU64,
    spinning: AtomicBool,
    origin: Instant,
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
        let state = shared.clone();
        let worker = thread::Builder::new()
            .name("evolve-activity".into())
            .spawn(move || {
                let mut previous = false;
                while !state.stop.load(Ordering::Acquire) {
                    let now = 1 + state.origin.elapsed().as_millis() as u64;
                    let active = recent(state.last.load(Ordering::Acquire), now);
                    state.spinning.store(active, Ordering::Release);
                    if active != previous {
                        crate::thread_priority::set_active(active);
                        previous = active;
                    }
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
    pub fn pulse(&self) {
        self.shared.last.store(
            1 + self.shared.origin.elapsed().as_millis() as u64,
            Ordering::Release,
        );
        crate::thread_priority::set_active(true);
    }
    pub fn spinning(&self) -> bool {
        self.shared.spinning.load(Ordering::Acquire)
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        crate::thread_priority::set_active(false);
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_expiry_does_not_count_an_unstarted_stream() {
        assert!(!recent(0, 1));
        assert!(recent(10, 109));
        assert!(!recent(10, 110));
    }
    #[test]
    fn live_thread_stops_spinning_when_callbacks_stop_and_joins() {
        let activity = Activity::new().unwrap();
        assert!(!activity.spinning());
        activity.pulse();
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
