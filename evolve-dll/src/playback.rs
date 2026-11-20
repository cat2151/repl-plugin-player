//! Sample-clock hold and linear crossfade between the two slots.
//!
//! Playback takes a slot only when the loader marked it ready. The next slot
//! sounds silently for a whole chord from the next chord, which fixes its
//! loudness trim ([`Level`]), before it may fade in. The current slot starts
//! the chord in progress at once and sounds even while it is measured, so
//! nothing waits for the first one. Only the current slot starts mid-chord: a
//! plugin that just reloaded a state on its instance can drop the notes of its
//! first block, and a held chord is not struck again. The current slot sounds
//! for at least `hold` and until the next one is ready and measured, then
//! fades. The faded-out slot is
//! handed back to the loader in `finish`, after its last render.
//!
//! The audio adapter calls `begin`, then `render` and `mix` per slot, then
//! `finish` per slot. None of them lock or allocate.
//!
//! A block misses its deadline when the host drops it (`begin` but no
//! `finish`) or when its last `finish` comes later than one block after
//! `begin`. A dropped block also skips the measurement, so a slot too heavy
//! to finish would otherwise keep both slots silent. A miss retires at once
//! the latest engaged slot still measuring, else the slot fading in. A miss
//! while the current slot sounds alone marks it instead: it fades out as soon
//! as the next one is measured, and while marked, misses retire nothing.
//! Retiring the current slot would leave silence until the next one is
//! measured, and blaming the next one would never let it be measured.
use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use repl_extension_api::sequence::Plan;
use sequence::Clock;

use crate::{
    level::{Level, Sample},
    ownership::{Ownership, ENGAGED, SLOT_COUNT},
};

/// Largest block `begin` blends; longer blocks keep the rest unscaled.
pub const CAPACITY: usize = 8192;
/// Shortest hold the engine accepts, in seconds.
pub const MIN_HOLD: f64 = 5.0;

struct Mix {
    current: Option<usize>,
    elapsed: u64,
    fade: Option<u64>,
    levels: [Level; SLOT_COUNT],
    enabled: [bool; SLOT_COUNT],
    retire: [bool; SLOT_COUNT],
    gains: [[f32; CAPACITY]; SLOT_COUNT],
    misses: Misses,
    /// The current slot missed while it sounded alone.
    doomed: Option<usize>,
}

impl Mix {
    /// The slots a deadline miss retires; see the module doc.
    fn blame(&mut self, heard: [bool; SLOT_COUNT]) -> [bool; SLOT_COUNT] {
        let mut skip = [false; SLOT_COUNT];
        let measuring = std::array::from_fn(|s| self.enabled[s] && !heard[s]);
        match self.current {
            Some(current) if self.doomed == Some(current) => {}
            _ if measuring.contains(&true) => skip = self.misses.latest(measuring),
            Some(current) if self.fade.is_some() => skip[1 - current] = true,
            Some(current) if heard[current] => self.doomed = Some(current),
            _ => {}
        }
        skip
    }
}

/// Deadline misses and which slot came last.
#[derive(Default)]
struct Misses {
    /// `begin` came and `finish` did not yet.
    open: bool,
    started: Option<Instant>,
    finished: Option<Instant>,
    period: Duration,
    block: u64,
    engaged_at: [u64; SLOT_COUNT],
}

impl Misses {
    /// Starts timing a block; returns whether the previous one missed.
    fn start(&mut self, frames: usize, rate: f64) -> bool {
        let now = Instant::now();
        self.block += 1;
        let dropped = std::mem::replace(&mut self.open, true);
        let late = match (self.started.replace(now), self.finished.take()) {
            (Some(started), Some(finished)) => finished - started > self.period,
            _ => false,
        };
        self.period = Duration::from_secs_f64(frames as f64 / rate);
        dropped || late
    }

    fn finish(&mut self) {
        self.open = false;
        self.finished = Some(Instant::now());
    }

    /// The enabled slots engaged last.
    fn latest(&self, enabled: [bool; SLOT_COUNT]) -> [bool; SLOT_COUNT] {
        let at = |s: usize| if enabled[s] { self.engaged_at[s] } else { 0 };
        let latest = (0..SLOT_COUNT).map(at).max().unwrap_or(0);
        std::array::from_fn(|s| enabled[s] && at(s) == latest)
    }
}

pub struct Playback {
    ownership: Arc<Ownership>,
    clocks: [Clock; SLOT_COUNT],
    hold: f64,
    fade: f64,
    chord_seconds: AtomicU64,
    armed: AtomicBool,
    gains: [AtomicU32; SLOT_COUNT],
    loudness: [AtomicU32; SLOT_COUNT],
    peaks: [AtomicU32; SLOT_COUNT],
    trims: [AtomicU32; SLOT_COUNT],
    fading_out: [AtomicBool; SLOT_COUNT],
    mix: UnsafeCell<Mix>,
}

impl Playback {
    pub fn new(hold: f64, fade: f64, ownership: Arc<Ownership>) -> Option<Self> {
        if !hold.is_finite() || hold < MIN_HOLD || !fade.is_finite() || fade <= 0.0 {
            return None;
        }
        Some(Self {
            ownership,
            clocks: Default::default(),
            hold,
            fade,
            chord_seconds: AtomicU64::new(1.0f64.to_bits()),
            armed: AtomicBool::new(false),
            gains: Default::default(),
            loudness: std::array::from_fn(|_| AtomicU32::new(f32::NAN.to_bits())),
            peaks: std::array::from_fn(|_| AtomicU32::new(f32::NAN.to_bits())),
            trims: std::array::from_fn(|_| AtomicU32::new(f32::NAN.to_bits())),
            fading_out: Default::default(),
            mix: UnsafeCell::new(Mix {
                current: None,
                elapsed: 0,
                fade: None,
                levels: Default::default(),
                enabled: [false; SLOT_COUNT],
                retire: [false; SLOT_COUNT],
                gains: [[0.0; CAPACITY]; SLOT_COUNT],
                misses: Misses::default(),
                doomed: None,
            }),
        })
    }

    /// Gain at the end of the last block.
    pub fn gain(&self, slot: usize) -> f32 {
        f32::from_bits(self.gains[slot].load(Ordering::Relaxed))
    }

    /// Loudness in LUFS, untrimmed peak in dBFS and the linear trim measured
    /// for the slot's patch; NaN until measured.
    pub fn measured(&self, slot: usize) -> (f32, f32, f32) {
        let read = |a: &AtomicU32| f32::from_bits(a.load(Ordering::Relaxed));
        (
            read(&self.loudness[slot]),
            read(&self.peaks[slot]),
            read(&self.trims[slot]),
        )
    }

    pub fn fading_out(&self, slot: usize) -> bool {
        self.fading_out[slot].load(Ordering::Relaxed)
    }

    /// Gives both slots the same chord pattern. Single producer.
    pub fn publish(&self, plan: &Plan) -> u32 {
        let mut result = 0;
        for (slot, clock) in self.clocks.iter().enumerate() {
            let mut plan = *plan;
            plan.track = slot as i32;
            result |= clock.publish(&plan);
        }
        if result == 0 {
            self.chord_seconds
                .store(plan.seconds.to_bits(), Ordering::Release);
            self.armed.store(true, Ordering::Release);
        }
        result
    }

    pub fn pending(&self) -> bool {
        self.clocks.iter().any(Clock::pending)
    }

    /// # Safety
    /// Audio callbacks of this engine are serialized.
    pub unsafe fn begin(&self, frames: u32, rate: f64) {
        let m = unsafe { &mut *self.mix.get() };
        let frames = (frames as usize).min(CAPACITY);
        m.enabled = [false; SLOT_COUNT];
        if !self.armed.load(Ordering::Acquire) {
            for gains in &mut m.gains {
                gains[..frames].fill(0.0);
            }
            return;
        }
        let prime =
            (f64::from_bits(self.chord_seconds.load(Ordering::Acquire)) * rate).ceil() as u64;
        let mut heard = [false; SLOT_COUNT];
        let missed = m.misses.start(frames, rate);
        for (slot, level) in m.levels.iter_mut().enumerate() {
            level.prepare(rate);
            if self.ownership.engage(slot) {
                m.misses.engaged_at[slot] = m.misses.block;
                if m.doomed == Some(slot) {
                    m.doomed = None;
                }
                level.reset();
                for value in [&self.loudness[slot], &self.peaks[slot], &self.trims[slot]] {
                    value.store(f32::NAN.to_bits(), Ordering::Relaxed);
                }
            }
            m.enabled[slot] = self.ownership.get(slot) == ENGAGED && !m.retire[slot];
            if m.enabled[slot] && level.settle(prime, m.current == Some(slot)) {
                let (loudness, peak) = (level.loudness() as f32, level.peak_db() as f32);
                self.loudness[slot].store(loudness.to_bits(), Ordering::Relaxed);
                self.peaks[slot].store(peak.to_bits(), Ordering::Relaxed);
                self.trims[slot].store((level.trim() as f32).to_bits(), Ordering::Relaxed);
            }
            heard[slot] = m.enabled[slot] && level.trimmed();
        }
        let skip = if missed {
            m.blame(heard)
        } else {
            [false; SLOT_COUNT]
        };
        for slot in (0..SLOT_COUNT).filter(|&s| skip[s]) {
            m.retire[slot] = true;
            m.enabled[slot] = false;
            heard[slot] = false;
            if m.current == Some(slot) {
                m.current = None;
                m.elapsed = 0;
            }
            m.fade = None;
        }
        if m.current.is_none() {
            m.current = (0..SLOT_COUNT).find(|&slot| m.enabled[slot]);
        }
        let hold = (self.hold * rate).round() as u64;
        let fade = (self.fade * rate).round().max(1.0) as u64;
        for frame in 0..frames {
            m.gains[0][frame] = 0.0;
            m.gains[1][frame] = 0.0;
            let Some(current) = m.current else {
                continue;
            };
            let next = 1 - current;
            let due = m.elapsed >= hold || m.doomed == Some(current);
            if m.fade.is_none() && due && heard[next] {
                m.fade = Some(0);
            }
            if let Some(position) = m.fade {
                let blend = (position as f64 / fade as f64).min(1.0) as f32;
                m.gains[current][frame] = 1.0 - blend;
                m.gains[next][frame] = blend;
                if position == fade {
                    m.retire[current] = true;
                    m.current = Some(next);
                    m.elapsed = 0;
                    m.fade = None;
                } else {
                    m.fade = Some(position + 1);
                }
            } else {
                m.gains[current][frame] = 1.0;
            }
            m.elapsed += 1;
        }
        if frames > 0 {
            for slot in 0..SLOT_COUNT {
                self.gains[slot].store(m.gains[slot][frames - 1].to_bits(), Ordering::Relaxed);
                let out = m.fade.is_some() && m.current == Some(slot);
                self.fading_out[slot].store(out, Ordering::Relaxed);
            }
        }
    }

    /// Emits the slot's note events; a retiring slot also releases every held
    /// note at the end of the block. Returns whether the slot sounds.
    ///
    /// # Safety
    /// Called between `begin` and `finish` of the same block.
    pub unsafe fn render(
        &self,
        slot: usize,
        frames: u32,
        rate: f64,
        mut emit: impl FnMut(u32, u8, bool, u16),
    ) -> bool {
        let m = unsafe { &mut *self.mix.get() };
        let level = &mut m.levels[slot];
        if m.enabled[slot] && m.current == Some(slot) {
            unsafe {
                self.clocks[slot].resume(|at, pitch, on, velocity| {
                    level.note_on();
                    emit(at, pitch, on, velocity);
                })
            };
        }
        unsafe {
            self.clocks[slot].render_enabled(
                slot as i32,
                frames,
                rate,
                m.enabled[slot],
                |at, pitch, on, velocity| {
                    if on {
                        level.note_on();
                    }
                    emit(at, pitch, on, velocity);
                },
            )
        };
        if m.retire[slot] && frames > 0 {
            let held = unsafe { self.clocks[slot].held() };
            for pitch in &held.notes[..held.count as usize] {
                emit(frames - 1, *pitch, false, 0);
            }
            unsafe { self.clocks[slot].release() };
        }
        m.enabled[slot]
    }

    /// Measures, trims, limits and fades every output channel of the slot,
    /// each `frames` long (at most `CAPACITY` of them are touched).
    ///
    /// # Safety
    /// Called once per slot after `begin` of the same block.
    pub unsafe fn mix<T: Sample>(&self, slot: usize, channels: &mut [&mut [T]], frames: usize) {
        let m = unsafe { &mut *self.mix.get() };
        let frames = channels
            .iter()
            .fold(frames.min(CAPACITY), |n, c| n.min(c.len()));
        m.levels[slot].process(channels, frames, &m.gains[slot]);
    }

    #[cfg(test)]
    pub fn crossfade(&self, slot: usize, frames: usize) -> Vec<f32> {
        unsafe { &*self.mix.get() }.gains[slot][..frames].to_vec()
    }

    /// Hands a faded-out slot back to the loader. The graph of that slot is
    /// not touched by audio after this.
    ///
    /// # Safety
    /// Called after the slot's last `render` of the block.
    pub unsafe fn finish(&self, slot: usize) {
        let m = unsafe { &mut *self.mix.get() };
        m.misses.finish();
        if m.retire[slot] {
            m.retire[slot] = false;
            m.enabled[slot] = false;
            self.ownership.retire(slot);
        }
    }
}

#[cfg(test)]
mod tests;
