//! Sample-clock hold and linear crossfade between the two slots of each group
//! ([`Layout`]).
//!
//! Playback takes a slot only when the loader marked it ready. In a `Chord`
//! group the next slot sounds silently for a whole chord from the next chord,
//! which fixes its loudness trim ([`Level`]), before it may fade in. The
//! current slot starts the chord in progress at once and sounds even while it
//! is measured, so nothing waits for the first one. Only the current slot
//! starts mid-chord: a plugin that just reloaded a state on its instance can
//! drop the notes of its first block, and a held chord is not struck again. A
//! `Timeline` group is not measured: its next slot may fade in once engaged.
//! The current slot sounds for at least `hold` and until the next one may fade
//! in, then fades. The faded-out slot is handed back to the loader in
//! `finish`, after its last render.
//!
//! The audio adapter calls `begin`, then `render` and `mix` per slot, then
//! `finish` per slot. None of them lock or allocate.
//!
//! A block misses its deadline when the host drops it (`begin` but no
//! `finish`) or when its last `finish` comes later than one block after
//! `begin`. A dropped block also skips the measurement, so a slot too heavy to
//! finish would otherwise keep a group silent. What a miss retires is in
//! [`misses`]. Retiring a current slot that sounds alone would leave silence
//! until the next one is measured, and blaming the next one would never let
//! it be measured, so such a slot is marked and fades out as soon as the next
//! one may fade in.
mod misses;

use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

use crate::clock::Clock;
use repl_extension_api::sequence::Plan;

use crate::{
    layout::{Input, Layout},
    level::{Level, Sample},
    ownership::{Ownership, ENGAGED},
};

use misses::{blame, Group, Misses};

/// Largest block `begin` blends; longer blocks keep the rest unscaled.
pub const CAPACITY: usize = 8192;
/// Shortest hold the engine accepts, in seconds.
pub const MIN_HOLD: f64 = 5.0;
/// `publish` got tracks that are not a chord group.
pub const NO_GROUP: u32 = 3;

struct Mix {
    groups: Box<[Group]>,
    levels: Box<[Level]>,
    enabled: Box<[bool]>,
    heard: Box<[bool]>,
    retire: Box<[bool]>,
    skip: Box<[bool]>,
    gains: Box<[[f32; CAPACITY]]>,
    misses: Misses,
}

/// The chord pattern a `Chord` group plays.
struct Chord {
    seconds: AtomicU64,
    armed: AtomicBool,
}

pub struct Playback {
    layout: Layout,
    ownership: Arc<Ownership>,
    clocks: Box<[Clock]>,
    chords: Box<[Chord]>,
    hold: f64,
    fade: f64,
    gains: Box<[AtomicU32]>,
    loudness: Box<[AtomicU32]>,
    peaks: Box<[AtomicU32]>,
    trims: Box<[AtomicU32]>,
    fading_out: Box<[AtomicBool]>,
    mix: UnsafeCell<Mix>,
}

fn atomics<T>(count: usize, make: impl Fn() -> T) -> Box<[T]> {
    (0..count).map(|_| make()).collect()
}

fn unmeasured() -> AtomicU32 {
    AtomicU32::new(f32::NAN.to_bits())
}

impl Playback {
    /// `ownership` has one entry per slot of `layout`.
    pub fn new(layout: Layout, hold: f64, fade: f64, ownership: Arc<Ownership>) -> Option<Self> {
        if !hold.is_finite() || hold < MIN_HOLD || !fade.is_finite() || fade <= 0.0 {
            return None;
        }
        let slots = layout.slot_count();
        let groups = layout.group_count();
        if ownership.len() != slots {
            return None;
        }
        Some(Self {
            ownership,
            clocks: atomics(slots, Clock::default),
            chords: atomics(groups, || Chord {
                seconds: AtomicU64::new(1.0f64.to_bits()),
                armed: AtomicBool::new(false),
            }),
            hold,
            fade,
            gains: atomics(slots, AtomicU32::default),
            loudness: atomics(slots, unmeasured),
            peaks: atomics(slots, unmeasured),
            trims: atomics(slots, unmeasured),
            fading_out: atomics(slots, AtomicBool::default),
            mix: UnsafeCell::new(Mix {
                groups: atomics(groups, Group::default),
                levels: atomics(slots, Level::default),
                enabled: vec![false; slots].into_boxed_slice(),
                heard: vec![false; slots].into_boxed_slice(),
                retire: vec![false; slots].into_boxed_slice(),
                skip: vec![false; slots].into_boxed_slice(),
                gains: vec![[0.0; CAPACITY]; slots].into_boxed_slice(),
                misses: Misses::new(slots),
            }),
            layout,
        })
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Gain at the end of the last block.
    pub fn gain(&self, slot: usize) -> f32 {
        f32::from_bits(self.gains[slot].load(Ordering::Relaxed))
    }

    /// Loudness in LUFS, untrimmed peak in dBFS and the linear trim measured
    /// for the slot's patch; NaN until measured and in `Timeline` groups.
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

    fn chord(&self, slot: usize) -> bool {
        self.layout.input(Layout::group_of(slot)) == Input::Chord
    }

    /// Gives both slots of the `Chord` group playing `tracks` the same chord
    /// pattern; `plan.track` is the group's first track. Single producer.
    /// Returns the clock's result, or [`NO_GROUP`].
    pub fn publish(&self, plan: &Plan, tracks: &[i32]) -> u32 {
        let group = (0..self.layout.group_count())
            .find(|&g| self.layout.input(g) == Input::Chord && self.layout.tracks(g) == tracks);
        let Some(group) = group.filter(|_| tracks.first() == Some(&plan.track)) else {
            return NO_GROUP;
        };
        let mut result = 0;
        for slot in Layout::slots(group) {
            let mut plan = *plan;
            plan.track = slot as i32;
            result |= self.clocks[slot].publish(&plan);
        }
        if result == 0 {
            let chord = &self.chords[group];
            chord
                .seconds
                .store(plan.seconds.to_bits(), Ordering::Release);
            chord.armed.store(true, Ordering::Release);
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
        let missed = m.misses.start(frames, rate);
        m.enabled.fill(false);
        m.heard.fill(false);
        for group in 0..self.layout.group_count() {
            let chord = self.layout.input(group) == Input::Chord;
            if chord && !self.chords[group].armed.load(Ordering::Acquire) {
                continue;
            }
            let seconds = f64::from_bits(self.chords[group].seconds.load(Ordering::Acquire));
            let prime = (seconds * rate).ceil() as u64;
            for slot in Layout::slots(group) {
                self.engage(m, slot, chord, rate, prime);
            }
        }
        if missed {
            blame(&m.misses, &mut m.groups, &m.enabled, &m.heard, &mut m.skip);
            for slot in (0..m.skip.len()).filter(|&s| m.skip[s]) {
                m.retire[slot] = true;
                m.enabled[slot] = false;
                m.heard[slot] = false;
                let group = &mut m.groups[Layout::group_of(slot)];
                if group.current == Some(slot) {
                    group.current = None;
                    group.elapsed = 0;
                }
                group.fade = None;
            }
        }
        let hold = (self.hold * rate).round() as u64;
        let fade = (self.fade * rate).round().max(1.0) as u64;
        for (index, group) in m.groups.iter_mut().enumerate() {
            let slots = Layout::slots(index);
            if group.current.is_none() {
                group.current = slots.clone().find(|&slot| m.enabled[slot]);
            }
            for slot in slots.clone() {
                m.gains[slot][..frames].fill(0.0);
            }
            for frame in 0..frames {
                crossfade(
                    group,
                    &mut m.gains,
                    &mut m.retire,
                    &m.heard,
                    frame,
                    hold,
                    fade,
                );
            }
            if frames > 0 {
                for slot in slots {
                    let gain = m.gains[slot][frames - 1];
                    self.gains[slot].store(gain.to_bits(), Ordering::Relaxed);
                    let out = group.fade.is_some() && group.current == Some(slot);
                    self.fading_out[slot].store(out, Ordering::Relaxed);
                }
            }
        }
    }

    /// Takes a ready slot, decides whether it sounds this block and, in a
    /// `Chord` group, measures it.
    fn engage(&self, m: &mut Mix, slot: usize, chord: bool, rate: f64, prime: u64) {
        let group = &mut m.groups[Layout::group_of(slot)];
        let level = &mut m.levels[slot];
        level.prepare(rate);
        if self.ownership.engage(slot) {
            m.misses.engaged(slot);
            if group.doomed == Some(slot) {
                group.doomed = None;
            }
            level.reset();
            for value in [&self.loudness[slot], &self.peaks[slot], &self.trims[slot]] {
                value.store(f32::NAN.to_bits(), Ordering::Relaxed);
            }
        }
        let enabled = self.ownership.get(slot) == ENGAGED && !m.retire[slot];
        m.enabled[slot] = enabled;
        if !chord {
            m.heard[slot] = enabled;
            return;
        }
        if enabled && level.settle(prime, group.current == Some(slot)) {
            let (loudness, peak) = (level.loudness() as f32, level.peak_db() as f32);
            self.loudness[slot].store(loudness.to_bits(), Ordering::Relaxed);
            self.peaks[slot].store(peak.to_bits(), Ordering::Relaxed);
            self.trims[slot].store((level.trim() as f32).to_bits(), Ordering::Relaxed);
        }
        m.heard[slot] = enabled && level.trimmed();
    }

    /// Emits the slot's note events; a retiring slot also releases every held
    /// note at the end of the block. A `Timeline` slot emits nothing. Returns
    /// whether the slot sounds.
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
        let enabled = m.enabled[slot];
        let current = m.groups[Layout::group_of(slot)].current == Some(slot);
        let level = &mut m.levels[slot];
        let clock = &self.clocks[slot];
        if enabled && current {
            unsafe {
                clock.resume(|at, pitch, on, velocity| {
                    level.note_on();
                    emit(at, pitch, on, velocity);
                })
            };
        }
        unsafe {
            clock.render_enabled(
                slot as i32,
                frames,
                rate,
                enabled,
                |at, pitch, on, velocity| {
                    if on {
                        level.note_on();
                    }
                    emit(at, pitch, on, velocity);
                },
            )
        };
        if m.retire[slot] && frames > 0 {
            let held = unsafe { clock.held() };
            for pitch in &held.notes[..held.count as usize] {
                emit(frames - 1, *pitch, false, 0);
            }
            unsafe { clock.release() };
        }
        enabled
    }

    /// Fades every output channel of the slot, each `frames` long (at most
    /// `CAPACITY` of them are touched). A `Chord` slot is also measured,
    /// trimmed and limited.
    ///
    /// # Safety
    /// Called once per slot after `begin` of the same block.
    pub unsafe fn mix<T: Sample>(&self, slot: usize, channels: &mut [&mut [T]], frames: usize) {
        let m = unsafe { &mut *self.mix.get() };
        let frames = channels
            .iter()
            .fold(frames.min(CAPACITY), |n, c| n.min(c.len()));
        let gains = &m.gains[slot];
        if self.chord(slot) {
            m.levels[slot].process(channels, frames, gains);
            return;
        }
        for samples in channels.iter_mut() {
            for (sample, gain) in samples[..frames].iter_mut().zip(gains) {
                *sample = T::put(sample.get() * f64::from(*gain));
            }
        }
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

/// One frame of a group's crossfade gains, over gains already zeroed.
fn crossfade(
    group: &mut Group,
    gains: &mut [[f32; CAPACITY]],
    retire: &mut [bool],
    heard: &[bool],
    frame: usize,
    hold: u64,
    fade: u64,
) {
    let Some(current) = group.current else {
        return;
    };
    let next = Layout::partner(current);
    let due = group.elapsed >= hold || group.doomed == Some(current);
    if group.fade.is_none() && due && heard[next] {
        group.fade = Some(0);
    }
    if let Some(position) = group.fade {
        let blend = (position as f64 / fade as f64).min(1.0) as f32;
        gains[current][frame] = 1.0 - blend;
        gains[next][frame] = blend;
        if position == fade {
            retire[current] = true;
            group.current = Some(next);
            group.elapsed = 0;
            group.fade = None;
        } else {
            group.fade = Some(position + 1);
        }
    } else {
        gains[current][frame] = 1.0;
    }
    group.elapsed += 1;
}

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod tests;
