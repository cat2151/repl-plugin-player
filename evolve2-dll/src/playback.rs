//! Sample-clock hold and linear crossfade between the two slots.
//!
//! Playback takes a slot only when the loader marked it ready, keeps the
//! current slot sounding for at least `hold` and until the next one is ready
//! and primed for a whole chord, then fades. The faded-out slot is handed back
//! to the loader in `finish`, after its last render.
//!
//! The audio adapter calls `begin`, then `render` and `mix`/`mix64` per slot,
//! then `finish` per slot. None of them lock or allocate.
use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

use repl_extension_api::sequence::Plan;
use sequence::Clock;

use crate::ownership::{Ownership, ENGAGED, SLOT_COUNT};

/// Largest block `begin` blends; longer blocks keep the rest unscaled.
pub const CAPACITY: usize = 8192;
/// Shortest hold the engine accepts, in seconds.
pub const MIN_HOLD: f64 = 5.0;

struct Mix {
    current: Option<usize>,
    elapsed: u64,
    fade: Option<u64>,
    prime: [u64; SLOT_COUNT],
    enabled: [bool; SLOT_COUNT],
    retire: [bool; SLOT_COUNT],
    gains: [[f32; CAPACITY]; SLOT_COUNT],
}

pub struct Playback {
    ownership: Arc<Ownership>,
    clocks: [Clock; SLOT_COUNT],
    hold: f64,
    fade: f64,
    chord_seconds: AtomicU64,
    armed: AtomicBool,
    gains: [AtomicU32; SLOT_COUNT],
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
            fading_out: Default::default(),
            mix: UnsafeCell::new(Mix {
                current: None,
                elapsed: 0,
                fade: None,
                prime: [0; SLOT_COUNT],
                enabled: [false; SLOT_COUNT],
                retire: [false; SLOT_COUNT],
                gains: [[0.0; CAPACITY]; SLOT_COUNT],
            }),
        })
    }

    /// Gain at the end of the last block.
    pub fn gain(&self, slot: usize) -> f32 {
        f32::from_bits(self.gains[slot].load(Ordering::Relaxed))
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
        for slot in 0..SLOT_COUNT {
            if self.ownership.engage(slot) {
                m.prime[slot] = 0;
            }
            m.enabled[slot] = self.ownership.get(slot) == ENGAGED && !m.retire[slot];
        }
        if m.current.is_none() {
            m.current = (0..SLOT_COUNT).find(|&slot| m.enabled[slot]);
        }
        let hold = (self.hold * rate).round() as u64;
        let fade = (self.fade * rate).round().max(1.0) as u64;
        let prime =
            (f64::from_bits(self.chord_seconds.load(Ordering::Acquire)) * rate).ceil() as u64;
        for frame in 0..frames {
            m.gains[0][frame] = 0.0;
            m.gains[1][frame] = 0.0;
            let Some(current) = m.current else {
                continue;
            };
            let next = 1 - current;
            for slot in 0..SLOT_COUNT {
                if m.enabled[slot] {
                    m.prime[slot] += 1;
                }
            }
            if m.fade.is_none() && m.elapsed >= hold && m.enabled[next] && m.prime[next] >= prime {
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
        let m = unsafe { &*self.mix.get() };
        unsafe {
            self.clocks[slot].render_enabled(slot as i32, frames, rate, m.enabled[slot], &mut emit)
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

    /// # Safety
    /// Called after `begin` of the same block.
    pub unsafe fn mix(&self, slot: usize, samples: &mut [f32]) {
        let m = unsafe { &*self.mix.get() };
        for (sample, gain) in samples.iter_mut().zip(&m.gains[slot]) {
            *sample *= gain;
        }
    }

    /// # Safety
    /// Called after `begin` of the same block.
    pub unsafe fn mix64(&self, slot: usize, samples: &mut [f64]) {
        let m = unsafe { &*self.mix.get() };
        for (sample, gain) in samples.iter_mut().zip(&m.gains[slot]) {
            *sample *= f64::from(*gain);
        }
    }

    /// Hands a faded-out slot back to the loader. The graph of that slot is
    /// not touched by audio after this.
    ///
    /// # Safety
    /// Called after the slot's last `render` of the block.
    pub unsafe fn finish(&self, slot: usize) {
        let m = unsafe { &mut *self.mix.get() };
        if m.retire[slot] {
            m.retire[slot] = false;
            m.enabled[slot] = false;
            self.ownership.retire(slot);
        }
    }
}

#[cfg(test)]
mod tests;
