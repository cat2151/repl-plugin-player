//! Fixed four-cycle Similarity scope, adapted from cat-plugin-player.
use crate::scope_capture::{Capture, Queue};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Display {
    pub samples: Vec<[f32; 2]>,
    pub note: u8,
    pub sample_rate: u32,
    pub shift_samples: f64,
}

struct Shared {
    stop: AtomicBool,
    display: Mutex<Option<Arc<Display>>>,
}

pub struct Scope {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl Scope {
    pub fn start(sample_rate: u32) -> Result<(Capture, Self), String> {
        let queue = Queue::new();
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            display: Mutex::new(None),
        });
        let worker_shared = Arc::clone(&shared);
        let worker_queue = Arc::clone(&queue);
        let worker = thread::Builder::new()
            .name("oscilloscope".into())
            .spawn(move || run(worker_queue, worker_shared, sample_rate))
            .map_err(|e| format!("cannot start oscilloscope: {e}"))?;
        Ok((
            Capture::new(queue),
            Self {
                shared,
                worker: Some(worker),
            },
        ))
    }

    pub fn display(&self) -> Option<Arc<Display>> {
        self.shared.display.try_lock().ok()?.clone()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(queue: Arc<Queue>, shared: Arc<Shared>, sample_rate: u32) {
    let mut serial = None;
    let mut stream = crate::scope_stream::Stream::new(sample_rate);
    while !shared.stop.load(Ordering::Relaxed) {
        // Bound each drain so continuous production cannot starve shutdown.
        for _ in 0..16_384 {
            let Some(sample) = queue.pop() else {
                break;
            };
            if serial.is_some_and(|s: u64| sample.serial != s.wrapping_add(1)) || sample.note == 128
            {
                *shared.display.lock().unwrap() = None;
            }
            serial = Some(sample.serial);
            if let Some(frame) = stream.sample(sample) {
                *shared.display.lock().unwrap() = Some(Arc::new(frame));
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn worker_publishes_four_cycle_stereo() {
        let (mut capture, scope) = Scope::start(8_000).unwrap();
        let audio: Vec<f32> = (0..512)
            .flat_map(|i| {
                let sample = (i as f32 * std::f32::consts::TAU * 440.0 / 8_000.0).sin();
                [sample, -sample]
            })
            .collect();
        capture.record(&audio, 2, &[0x20904564]);
        let deadline = Instant::now() + Duration::from_secs(2);
        while scope.display().is_none() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        let frame = scope.display().unwrap();
        assert_eq!(frame.note, 69);
        assert_eq!(frame.samples.len(), crate::scope_trigger::width(8_000, 69));
        assert!(frame.samples.iter().all(|s| s[0] == -s[1]));
    }
}
