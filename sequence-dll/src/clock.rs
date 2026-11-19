use repl_extension_api::sequence::{Plan, Step};
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

#[derive(Default)]
struct Playback {
    plan: Plan,
    held: Step,
    step: usize,
    frame: f64,
    next: f64,
    release: f64,
}

#[derive(Default)]
pub struct Clock {
    queue: [UnsafeCell<Plan>; 4],
    written: AtomicU32,
    read: AtomicU32,
    playback: UnsafeCell<Playback>,
}

impl Clock {
    pub fn pending(&self) -> bool {
        self.written.load(Ordering::Acquire) != self.read.load(Ordering::Acquire)
    }
    pub fn publish(&self, plan: &Plan) -> u32 {
        if !(1..=64).contains(&plan.count)
            || plan.track < 0
            || plan.velocity == 0
            || !plan.seconds.is_finite()
            || !(0.05..=3600.0).contains(&plan.seconds)
            || !plan.gate.is_finite()
            || !(0.0 < plan.gate && plan.gate <= 1.0)
            || plan.steps[..plan.count as usize].iter().any(|step| {
                !(1..=8).contains(&step.count)
                    || step.notes[..step.count as usize]
                        .iter()
                        .enumerate()
                        .any(|(i, pitch)| *pitch > 127 || step.notes[..i].contains(pitch))
            })
        {
            return 1;
        }
        let w = self.written.load(Ordering::Relaxed);
        if w.wrapping_sub(self.read.load(Ordering::Acquire)) == 4 {
            return 2;
        }
        // Single producer; the consumer releases the slot only after copying it.
        unsafe {
            *self.queue[w as usize % 4].get() = *plan;
        }
        self.written.store(w.wrapping_add(1), Ordering::Release);
        0
    }

    /// # Safety
    /// Audio callbacks of this clock are excluded; the UAPMD adapter does so
    /// for lifecycle operations.
    pub unsafe fn reset(&self) {
        self.read
            .store(self.written.load(Ordering::Acquire), Ordering::Release);
        unsafe {
            *self.playback.get() = Playback::default();
        }
    }
    /// # Safety
    /// Audio callbacks of this clock are excluded.
    pub unsafe fn rewind(&self) {
        let state = unsafe { &mut *self.playback.get() };
        state.held = Step::default();
        state.step = 0;
        state.frame = 0.0;
        state.next = 0.0;
        state.release = 0.0;
    }
    /// # Safety
    /// The caller serializes the audio callbacks of this clock.
    pub unsafe fn held(&self) -> Step {
        unsafe { (*self.playback.get()).held }
    }

    /// The host reset the synth, but the sequence must retain its transport phase.
    ///
    /// # Safety
    /// The caller serializes the audio callbacks of this clock.
    pub unsafe fn release(&self) {
        unsafe { (*self.playback.get()).held = Step::default() };
    }

    fn adopt(&self, state: &mut Playback) {
        let mut r = self.read.load(Ordering::Relaxed);
        let w = self.written.load(Ordering::Acquire);
        while r != w {
            state.plan = unsafe { *self.queue[r as usize % 4].get() };
            r = r.wrapping_add(1);
        }
        self.read.store(r, Ordering::Release);
    }

    #[cfg(test)]
    /// # Safety
    /// The caller serializes the audio callbacks of this clock.
    pub unsafe fn render(
        &self,
        track: i32,
        frames: u32,
        rate: f64,
        mut emit: impl FnMut(u32, u8, bool, u16),
    ) {
        unsafe { self.render_enabled(track, frames, rate, true, &mut emit) };
    }

    /// # Safety
    /// The caller serializes the audio callbacks of this clock.
    pub unsafe fn render_enabled(
        &self,
        track: i32,
        frames: u32,
        rate: f64,
        enabled: bool,
        mut emit: impl FnMut(u32, u8, bool, u16),
    ) {
        let mut emit = |offset, pitch, on, velocity| {
            if enabled {
                emit(offset, pitch, on, velocity);
            }
        };
        if frames == 0 || !rate.is_finite() || rate <= 0.0 {
            return;
        }
        // The caller serializes callbacks for the owning track.
        let state = unsafe { &mut *self.playback.get() };
        if state.plan.count == 0 {
            self.adopt(state);
        }
        if track != state.plan.track || state.plan.count == 0 {
            return;
        }
        let end = state.frame + f64::from(frames);
        loop {
            let off = state.held.count > 0 && state.release < state.next;
            let at = if off { state.release } else { state.next };
            if at >= end {
                break;
            }
            let offset = (at - state.frame).round().clamp(0.0, f64::from(frames - 1)) as u32;
            if off {
                for &pitch in &state.held.notes[..state.held.count as usize] {
                    emit(offset, pitch, false, 0);
                }
                state.held = Step::default();
            } else {
                self.adopt(state);
                let next = state.plan.steps[state.step % state.plan.count as usize];
                for &pitch in &state.held.notes[..state.held.count as usize] {
                    if !next.notes[..next.count as usize].contains(&pitch) {
                        emit(offset, pitch, false, 0);
                    }
                }
                for &pitch in &next.notes[..next.count as usize] {
                    if !state.held.notes[..state.held.count as usize].contains(&pitch) {
                        emit(offset, pitch, true, state.plan.velocity);
                    }
                }
                state.held = next;
                state.step = (state.step + 1) % state.plan.count as usize;
                let period = state.plan.seconds * rate;
                state.release = state.next + period * state.plan.gate;
                state.next += period;
            }
        }
        state.frame = end;
        // A bypassed synth missed these notes; prime every voice on its next chord.
        if !enabled {
            state.held = Step::default();
        }
    }
}
