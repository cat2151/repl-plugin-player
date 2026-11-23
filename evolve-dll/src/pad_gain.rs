//! Peak correction at the pad instances and limiting at the final mix.
mod measurement;
#[cfg(test)]
mod tests;

use crate::level::{Level, Sample, MAX_CHANNELS};
use measurement::Measurement;
use std::{
    cell::RefCell,
    ffi::c_void,
    sync::{
        atomic::{AtomicI32, AtomicU64, Ordering::SeqCst},
        Mutex,
    },
};

const MAX_INSTANCES: usize = 16;
const UPDATING: u64 = 1;
const ENABLED: u64 = 2;
const MASTER: i32 = -2;
const GLIDE_SECONDS: f64 = 0.05;

struct Slot {
    instance: AtomicI32,
    state: Mutex<Option<Instance>>,
    meter: Meter,
}

static SLOTS: [Slot; MAX_INSTANCES] = [const {
    Slot {
        instance: AtomicI32::new(-1),
        state: Mutex::new(None),
        meter: Meter::new(),
    }
}; MAX_INSTANCES];
// The control thread publishes the complete set between odd and even revisions.
static REVISION: AtomicU64 = AtomicU64::new(0);
static MASTER_METER: Meter = Meter::new();

// Publish a block's applied gain and its session together so a reader cannot
// mistake an in-flight block from the previous session for a current one.
struct Meter(AtomicU64);

impl Meter {
    const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    fn publish(&self, revision: u64, gain: f32) {
        let bits = (revision << 32) | u64::from(gain.to_bits());
        self.0.store(bits, std::sync::atomic::Ordering::Relaxed);
    }

    fn read(&self, revision: u64) -> f32 {
        let bits = self.0.load(std::sync::atomic::Ordering::Relaxed);
        if (bits >> 32) as u32 == revision as u32 {
            f32::from_bits(bits as u32)
        } else {
            f32::NAN
        }
    }
}

// Called on the control thread. NaN means no sample for this session.
#[no_mangle]
pub extern "C" fn repl_pad_gain_read(instance: i32) -> f32 {
    let revision = REVISION.load(SeqCst);
    if revision & (UPDATING | ENABLED) != ENABLED {
        return f32::NAN;
    }
    if instance == MASTER {
        MASTER_METER.read(revision)
    } else if instance >= 0 {
        SLOTS
            .iter()
            .find(|slot| slot.instance.load(SeqCst) == instance)
            .map_or(f32::NAN, |slot| slot.meter.read(revision))
    } else {
        f32::NAN
    }
}

thread_local! {
    static LIMITER: RefCell<Option<(u64, f64, Level)>> = const { RefCell::new(None) };
}

// Called only by the control thread. An empty list disables the session.
#[no_mangle]
pub unsafe extern "C" fn repl_pad_gain_set(ids: *const i32, count: usize) -> bool {
    if count > MAX_INSTANCES || (count != 0 && ids.is_null()) {
        return false;
    }
    let ids = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ids, count) }
    };
    if ids
        .iter()
        .enumerate()
        .any(|(i, id)| *id < 0 || ids[..i].contains(id))
    {
        return false;
    }
    let next = (REVISION.load(SeqCst) & !3).wrapping_add(4);
    REVISION.store(next | UPDATING, SeqCst);
    for (i, slot) in SLOTS.iter().enumerate() {
        slot.instance
            .store(ids.get(i).copied().unwrap_or(-1), SeqCst);
    }
    REVISION.store(next | if ids.is_empty() { 0 } else { ENABLED }, SeqCst);
    true
}

struct Instance {
    revision: u64,
    rate: f64,
    measurement: Measurement,
    gain: f64,
}

impl Instance {
    fn new(revision: u64, rate: f64) -> Self {
        Self {
            revision,
            rate,
            measurement: Measurement::new(rate),
            gain: 1.0,
        }
    }

    fn process<T: Sample>(&mut self, channels: &mut [&mut [T]], frames: usize, events: &[u32]) {
        self.measurement.events(events);
        let peak = channels
            .iter()
            .flat_map(|samples| samples.iter())
            .map(|sample| sample.get().abs())
            .filter(|peak| peak.is_finite())
            .fold(0.0, f64::max);
        let target = self.measurement.observe(peak, frames, self.gain);
        // Apply a lower ceiling before this block's peak reaches the mix.
        self.gain = self.gain.min(target);
        let next = self.gain
            + (target - self.gain) * (1.0 - (-(frames as f64) / (self.rate * GLIDE_SECONDS)).exp());
        let step = (next - self.gain) / frames as f64;
        for samples in channels {
            let mut gain = self.gain;
            for sample in samples.iter_mut() {
                let raw = sample.get();
                gain += step;
                if raw.is_finite() {
                    *sample = T::put(raw * gain);
                } else {
                    *sample = T::put(0.0);
                }
            }
        }
        self.gain = next;
    }
}

fn limit<T: Sample>(level: &mut Level, channels: &mut [&mut [T]], frames: usize) -> f32 {
    let mut gain = 1.0;
    for frame in 0..frames {
        let mut peak: f64 = 0.0;
        for samples in channels.iter_mut() {
            let sample = &mut samples[frame];
            let raw = sample.get();
            if raw.is_finite() {
                peak = peak.max(raw.abs());
            } else {
                *sample = T::put(0.0);
            }
        }
        gain = level.limit(peak);
        for samples in channels.iter_mut() {
            samples[frame] = T::put(samples[frame].get() * gain);
        }
    }
    gain as f32
}

struct Block<'a> {
    frames: usize,
    rate: f64,
    revision: u64,
    events: &'a [u32],
    slot: Option<&'a Slot>,
}

impl Block<'_> {
    unsafe fn process<T: Sample>(&self, out: *const *mut T, count: usize) {
        let mut channels: [&mut [T]; MAX_CHANNELS] = Default::default();
        let mut used = 0;
        for i in 0..count {
            let samples = unsafe { *out.add(i) };
            if !samples.is_null() {
                channels[used] = unsafe { std::slice::from_raw_parts_mut(samples, self.frames) };
                used += 1;
            }
        }
        if used == 0 {
            return;
        }
        let channels = &mut channels[..used];
        if let Some(slot) = self.slot {
            // A track can change workers. Keep its state with the slot and never wait.
            let Ok(mut state) = slot.state.try_lock() else {
                return;
            };
            if REVISION.load(SeqCst) != self.revision {
                return;
            }
            if state
                .as_ref()
                .is_none_or(|s| s.revision != self.revision || s.rate != self.rate)
            {
                *state = Some(Instance::new(self.revision, self.rate));
            }
            if let Some(instance) = state.as_mut() {
                instance.process(channels, self.frames, self.events);
                slot.meter.publish(self.revision, instance.gain as f32);
            }
        } else {
            LIMITER.with_borrow_mut(|state| {
                if state
                    .as_ref()
                    .is_none_or(|(rev, rate, _)| *rev != self.revision || *rate != self.rate)
                {
                    let mut level = Level::default();
                    level.prepare(self.rate);
                    *state = Some((self.revision, self.rate, level));
                }
                if let Some((_, _, level)) = state.as_mut() {
                    MASTER_METER.publish(self.revision, limit(level, channels, self.frames));
                }
            });
        }
    }
}

// UAPMD supplies disjoint writable planar buffers and the actual delivered MIDI block.
#[no_mangle]
pub unsafe extern "C" fn repl_pad_gain_process(
    instance: i32,
    out: *const *mut c_void,
    count: usize,
    frames: usize,
    rate: f64,
    wide: bool,
    events: *const u32,
    words: usize,
) {
    let revision = REVISION.load(SeqCst);
    if revision & (UPDATING | ENABLED) != ENABLED
        || frames == 0
        || !(1..=MAX_CHANNELS).contains(&count)
        || !rate.is_finite()
        || rate <= 0.0
    {
        return;
    }
    let slot = if instance == MASTER {
        None
    } else if instance >= 0 {
        let Some(slot) = SLOTS
            .iter()
            .find(|slot| slot.instance.load(SeqCst) == instance)
        else {
            return;
        };
        Some(slot)
    } else {
        return;
    };
    let events = if words == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(events, words) }
    };
    let block = Block {
        frames,
        rate,
        revision,
        events,
        slot,
    };
    if wide {
        unsafe { block.process::<f64>(out.cast(), count) };
    } else {
        unsafe { block.process::<f32>(out.cast(), count) };
    }
}
