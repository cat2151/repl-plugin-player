//! Bounded single-producer/single-consumer transport. All storage is allocated
//! before playback; the audio callback only performs atomic loads and stores.
use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

const CAPACITY: usize = 16_384;

#[derive(Clone, Copy)]
pub struct Sample {
    pub stereo: [f32; 2],
    pub note: u8,
    pub epoch: u64,
    pub serial: u64,
}

struct Slot {
    audio: AtomicU64,
    note: AtomicU8,
    epoch: AtomicU64,
    serial: AtomicU64,
}

pub struct Queue {
    slots: Box<[Slot]>,
    read: AtomicUsize,
    write: AtomicUsize,
}

impl Queue {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            slots: (0..CAPACITY)
                .map(|_| Slot {
                    audio: AtomicU64::new(0),
                    note: AtomicU8::new(128),
                    epoch: AtomicU64::new(0),
                    serial: AtomicU64::new(0),
                })
                .collect(),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        })
    }

    // Called only by Capture on the audio thread.
    fn push(&self, sample: Sample) {
        let write = self.write.load(Ordering::Relaxed);
        if write.wrapping_sub(self.read.load(Ordering::Acquire)) >= CAPACITY {
            return;
        }
        let slot = &self.slots[write % CAPACITY];
        slot.audio.store(
            u64::from(sample.stereo[0].to_bits()) | (u64::from(sample.stereo[1].to_bits()) << 32),
            Ordering::Relaxed,
        );
        slot.note.store(sample.note, Ordering::Relaxed);
        slot.epoch.store(sample.epoch, Ordering::Relaxed);
        slot.serial.store(sample.serial, Ordering::Relaxed);
        self.write.store(write.wrapping_add(1), Ordering::Release);
    }

    // Called only by the scope worker. Release prevents slot reuse until read.
    pub fn pop(&self) -> Option<Sample> {
        let read = self.read.load(Ordering::Relaxed);
        if read == self.write.load(Ordering::Acquire) {
            return None;
        }
        let slot = &self.slots[read % CAPACITY];
        let audio = slot.audio.load(Ordering::Relaxed);
        let sample = Sample {
            stereo: [
                f32::from_bits(audio as u32),
                f32::from_bits((audio >> 32) as u32),
            ],
            note: slot.note.load(Ordering::Relaxed),
            epoch: slot.epoch.load(Ordering::Relaxed),
            serial: slot.serial.load(Ordering::Relaxed),
        };
        self.read.store(read.wrapping_add(1), Ordering::Release);
        Some(sample)
    }
}

pub struct Capture {
    queue: Arc<Queue>,
    note: u8,
    epoch: u64,
    serial: u64,
}

impl Capture {
    pub fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue,
            note: 128,
            epoch: 0,
            serial: 0,
        }
    }

    pub fn reset(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
    }

    #[cfg(test)]
    pub fn record(&mut self, audio: &[f32], channels: usize, events: &[u32]) {
        self.record_samples(audio.len() / channels, events, |frame| {
            let i = frame * channels;
            [audio[i], audio[i + usize::from(channels > 1)]]
        });
    }

    // UAPMD JR timestamps are absolute offsets within the current audio block.
    fn record_samples(
        &mut self,
        frames: usize,
        events: &[u32],
        mut audio: impl FnMut(usize) -> [f32; 2],
    ) {
        const LENGTHS: [usize; 16] = [1, 1, 1, 2, 2, 4, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4];
        let mut event = 0;
        let mut at = 0;
        for frame in 0..frames {
            while event < events.len() {
                let word = events[event];
                let length = LENGTHS[(word >> 28) as usize];
                if length > events.len() - event {
                    break;
                }
                if word & 0xFFFF_0000 == 0x0020_0000 {
                    at = (word & 0xFFFF) as usize;
                    event += 1;
                    continue;
                }
                if at > frame {
                    break;
                }
                let kind = word >> 28;
                let velocity = if kind == 4 {
                    events[event + 1] >> 16
                } else {
                    word & 127
                };
                if (kind == 2 || kind == 4) && (word >> 20) & 15 == 9 && velocity != 0 {
                    self.note = ((word >> 8) & 127) as u8;
                    self.reset();
                }
                event += length;
            }
            let clean = |v: f32| if v.is_finite() { v } else { 0.0 };
            self.queue.push(Sample {
                stereo: audio(frame).map(clean),
                note: self.note,
                epoch: self.epoch,
                serial: self.serial,
            });
            self.serial = self.serial.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_changes_at_timestamp_and_note_off_retains_tail_reference() {
        let queue = Queue::new();
        let mut capture = Capture::new(Arc::clone(&queue));
        capture.record(
            &[0.1, 0.2, 0.3, 0.4],
            1,
            &[0x20903064, 0x0020_0002, 0x20903c64],
        );
        let first = queue.pop().unwrap();
        assert_eq!(first.stereo, [0.1, 0.1]);
        assert_eq!(first.note, 48);
        assert_eq!(queue.pop().unwrap().note, 48);
        let third = queue.pop().unwrap();
        assert_eq!(third.note, 60);
        assert_ne!(first.epoch, third.epoch);
        queue.pop();
        capture.record(&[0.5], 1, &[0x20803c00]);
        assert_eq!(queue.pop().unwrap().note, 60);
    }

    #[test]
    fn native_double_buffers_and_absolute_ump_offsets_keep_note_alignment() {
        let queue = Queue::new();
        let mut capture = Capture::new(Arc::clone(&queue));
        let left = [0.1_f64, 0.2, f64::NAN, 0.4, 0.5];
        let right = [-0.1_f64, -0.2, -0.3, -0.4, -0.5];
        // Two-word non-note messages must not interpret their payload as Note On.
        let events = [
            0x0020_0001,
            0x4090_3c00,
            0xffff_0000,
            0x4080_3c00,
            0x2090_7f64,
            0x0020_0003,
            0x2090_4864,
        ];
        unsafe {
            record_native(
                &mut capture,
                left.as_ptr().cast(),
                right.as_ptr().cast(),
                left.len(),
                1,
                events.as_ptr(),
                events.len(),
            );
        }
        let samples: Vec<_> = (0..5).map(|_| queue.pop().unwrap()).collect();
        assert_eq!(
            samples.iter().map(|s| s.note).collect::<Vec<_>>(),
            [128, 60, 60, 72, 72]
        );
        assert_eq!(samples[2].stereo, [0.0, -0.3]);
        assert_eq!(samples[4].stereo, [0.5, -0.5]);
    }

    #[test]
    fn overflow_is_detectable_instead_of_joining_discontinuous_audio() {
        let queue = Queue::new();
        let mut capture = Capture::new(Arc::clone(&queue));
        capture.record(&vec![1.0; CAPACITY + 10], 1, &[]);
        for expected in 0..CAPACITY {
            assert_eq!(queue.pop().unwrap().serial, expected as u64);
        }
        capture.record(&[1.0], 1, &[]);
        assert_eq!(queue.pop().unwrap().serial, (CAPACITY + 10) as u64);
    }

    #[test]
    fn concurrent_transport_keeps_stereo_and_metadata_together_across_wraps() {
        use std::sync::atomic::AtomicBool;
        let queue = Queue::new();
        let finished = Arc::new(AtomicBool::new(false));
        let producer_queue = Arc::clone(&queue);
        let producer_finished = Arc::clone(&finished);
        let producer = std::thread::spawn(move || {
            let mut capture = Capture::new(Arc::clone(&producer_queue));
            for block in 0..256 {
                // Test-only pacing ensures every sample is delivered regardless
                // of OS scheduling. Overflow behavior is covered separately.
                while producer_queue
                    .write
                    .load(Ordering::Relaxed)
                    .wrapping_sub(producer_queue.read.load(Ordering::Acquire))
                    > CAPACITY - 256
                {
                    std::thread::yield_now();
                }
                let mut audio = [0.0; 512];
                for (i, stereo) in audio.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                    let value = (block * 256 + i) as f32;
                    stereo.copy_from_slice(&[value, -value]);
                }
                capture.record(&audio, 2, &[0x20903c64]);
                std::thread::yield_now();
            }
            producer_finished.store(true, Ordering::Release);
        });
        let mut previous = None;
        loop {
            // Observe completion before pop, so an empty queue after completion
            // cannot race with the producer's final publication.
            let done = finished.load(Ordering::Acquire);
            if let Some(sample) = queue.pop() {
                assert_eq!(
                    sample.stereo,
                    [sample.serial as f32, -(sample.serial as f32)]
                );
                assert_eq!(sample.note, 60);
                assert_eq!(sample.epoch, sample.serial / 256 + 1);
                assert!(previous.is_none_or(|serial| sample.serial > serial));
                previous = Some(sample.serial);
            } else if done {
                break;
            } else {
                std::thread::yield_now();
            }
        }
        producer.join().unwrap();
        assert_eq!(previous, Some(256 * 256 - 1));
    }
}

// Called only by the selected plugin's processing thread. Native close waits for
// processing to finish before Bridge releases this producer. Buffers are planar,
// remain valid for this call, and contain `frames` f32/f64 samples per channel.
pub type Callback = unsafe extern "C" fn(
    *mut Capture,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    usize,
    u32,
    *const u32,
    usize,
);

pub unsafe extern "C" fn record_native(
    capture: *mut Capture,
    left: *const std::ffi::c_void,
    right: *const std::ffi::c_void,
    frames: usize,
    doubles: u32,
    events: *const u32,
    words: usize,
) {
    let events = if words == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(events, words) }
    };
    let capture = unsafe { &mut *capture };
    capture.record_samples(frames, events, |i| unsafe {
        if doubles != 0 {
            [
                *left.cast::<f64>().add(i) as f32,
                *right.cast::<f64>().add(i) as f32,
            ]
        } else {
            [*left.cast::<f32>().add(i), *right.cast::<f32>().add(i)]
        }
    });
}
