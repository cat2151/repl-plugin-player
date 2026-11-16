//! A/B sample-clock ownership and linear crossfade. No host locks or allocation
//! on the render path. The adapter calls begin, both renders/mixes, then finish.
use crate::clock::Clock;
use repl_extension_api::sequence::Plan;
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

pub const NEED: u32 = 0;
const LOADING: u32 = 1;
const READY: u32 = 2;
const ENGAGED: u32 = 3;
const WAITING: u32 = 4;
pub const CAPACITY: usize = 8192;

struct Playback {
    current: Option<usize>,
    elapsed: u64,
    fade: Option<u64>,
    prime: [u64; 2],
    enabled: [bool; 2],
    retire: [bool; 2],
    gains: [[f32; CAPACITY]; 2],
}

pub struct Evolution {
    status: [AtomicU32; 2],
    clocks: [Clock; 2],
    hold: f64,
    fade: f64,
    chord_seconds: std::sync::atomic::AtomicU64,
    armed: AtomicBool,
    gains: [AtomicU32; 2],
    playback: UnsafeCell<Playback>,
}

impl Evolution {
    pub fn new(hold: f64, fade: f64) -> Option<Self> {
        if !hold.is_finite() || hold < 5.0 || !fade.is_finite() || fade <= 0.0 {
            return None;
        }
        Some(Self {
            status: [AtomicU32::new(NEED), AtomicU32::new(WAITING)],
            clocks: Default::default(),
            hold,
            fade,
            chord_seconds: std::sync::atomic::AtomicU64::new(1.0f64.to_bits()),
            armed: AtomicBool::new(false),
            gains: [AtomicU32::new(0), AtomicU32::new(0)],
            playback: UnsafeCell::new(Playback {
                current: None,
                elapsed: 0,
                fade: None,
                prime: [0; 2],
                enabled: [false; 2],
                retire: [false; 2],
                gains: [[0.0; CAPACITY]; 2],
            }),
        })
    }
    pub fn status(&self, slot: usize) -> u32 {
        self.status[slot].load(Ordering::Acquire)
    }
    pub fn gain(&self, slot: usize) -> f32 {
        f32::from_bits(self.gains[slot].load(Ordering::Relaxed))
    }
    pub fn claim(&self, slot: usize) -> bool {
        self.status[slot]
            .compare_exchange(NEED, LOADING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub fn ready(&self, slot: usize, success: bool) {
        self.status[slot].store(if success { READY } else { NEED }, Ordering::Release);
    }
    pub fn publish(&self, plan: &Plan) -> u32 {
        // Both queues have identical producer/consumer cadence.
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
    pub unsafe fn begin(&self, frames: u32, rate: f64) {
        let p = unsafe { &mut *self.playback.get() };
        p.enabled = [false; 2];
        if !self.armed.load(Ordering::Acquire) {
            for gains in &mut p.gains {
                gains[..frames as usize].fill(0.0);
            }
            return;
        }
        for slot in 0..2 {
            if self.status(slot) == READY {
                p.prime[slot] = 0;
                self.status[slot].store(ENGAGED, Ordering::Release);
            }
            p.enabled[slot] = self.status(slot) == ENGAGED && !p.retire[slot];
        }
        if p.current.is_none() && p.enabled[0] {
            p.current = Some(0);
            // Request B as soon as A starts; no startup wait for B.
            self.status[1].store(NEED, Ordering::Release);
        }
        let hold = (self.hold * rate).round() as u64;
        let fade = (self.fade * rate).round().max(1.0) as u64;
        let prime =
            (f64::from_bits(self.chord_seconds.load(Ordering::Acquire)) * rate).ceil() as u64;
        for frame in 0..frames as usize {
            p.gains[0][frame] = 0.0;
            p.gains[1][frame] = 0.0;
            let Some(current) = p.current else {
                continue;
            };
            let next = 1 - current;
            for slot in 0..2 {
                if p.enabled[slot] {
                    p.prime[slot] += 1;
                }
            }
            if p.fade.is_none() && p.elapsed >= hold && p.enabled[next] && p.prime[next] >= prime {
                p.fade = Some(0);
            }
            if let Some(position) = p.fade {
                let blend = (position as f64 / fade as f64).min(1.0) as f32;
                p.gains[current][frame] = 1.0 - blend;
                p.gains[next][frame] = blend;
                if position == fade {
                    p.retire[current] = true;
                    p.current = Some(next);
                    p.elapsed = 0;
                    p.fade = None;
                } else {
                    p.fade = Some(position + 1);
                }
            } else {
                p.gains[current][frame] = 1.0;
            }
            p.elapsed += 1;
        }
        if frames > 0 {
            for slot in 0..2 {
                self.gains[slot].store(
                    p.gains[slot][frames as usize - 1].to_bits(),
                    Ordering::Relaxed,
                );
            }
        }
    }
    pub unsafe fn render(
        &self,
        slot: usize,
        frames: u32,
        rate: f64,
        mut emit: impl FnMut(u32, u8, bool, u16),
    ) -> bool {
        let p = unsafe { &*self.playback.get() };
        unsafe {
            self.clocks[slot].render_enabled(slot as i32, frames, rate, p.enabled[slot], &mut emit)
        };
        if p.retire[slot] {
            let held = unsafe { self.clocks[slot].held() };
            for pitch in &held.notes[..held.count as usize] {
                emit(frames - 1, *pitch, false, 0);
            }
            unsafe { self.clocks[slot].release() };
        }
        p.enabled[slot]
    }
    pub unsafe fn mix(&self, slot: usize, samples: &mut [f32]) {
        let p = unsafe { &*self.playback.get() };
        for (sample, gain) in samples.iter_mut().zip(&p.gains[slot]) {
            *sample *= gain;
        }
    }
    pub unsafe fn mix64(&self, slot: usize, samples: &mut [f64]) {
        let p = unsafe { &*self.playback.get() };
        for (sample, gain) in samples.iter_mut().zip(&p.gains[slot]) {
            *sample *= f64::from(*gain);
        }
    }
    pub unsafe fn finish(&self, slot: usize) {
        let p = unsafe { &mut *self.playback.get() };
        if p.retire[slot] {
            p.retire[slot] = false;
            p.enabled[slot] = false;
            // No graph access after this release; control may now destroy it.
            self.status[slot].store(NEED, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests;
