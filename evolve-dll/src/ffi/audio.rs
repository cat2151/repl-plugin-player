//! `before` and `after`, called by the adapter around each track's processing
//! on the audio thread. They find the slot of the track and neither lock nor
//! allocate.
use std::ffi::c_void;

use crate::{
    layout::Input,
    level::{Sample, MAX_CHANNELS},
};

use super::{get, Engine};

/// UMP ticks per second of the delta clockstamps.
const TICKS_PER_SECOND: f64 = 31_250.0;

/// Writes note events as UMP words: a delta clockstamp, then a MIDI 2.0 note
/// on or off. Events past `capacity` are dropped.
struct Ump {
    events: *mut u8,
    capacity: usize,
    end: usize,
    rate: f64,
    ticks: u32,
}

impl Ump {
    fn push(&mut self, offset: u32, pitch: u8, on: bool, velocity: u16) {
        let ticks = (f64::from(offset) * TICKS_PER_SECOND / self.rate).round() as u32;
        let status: u32 = if on { 0x4090_0000 } else { 0x4080_0000 };
        let words = [
            0x0020_0000 | ticks.wrapping_sub(self.ticks),
            status | u32::from(pitch) << 8,
            u32::from(velocity) << 16,
        ];
        let size = size_of_val(&words);
        if self.events.is_null() || self.end + size > self.capacity {
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(words.as_ptr().cast(), self.events.add(self.end), size)
        };
        self.end += size;
        self.ticks = ticks;
    }
}

pub(super) unsafe extern "C" fn before(
    h: *mut c_void,
    track: i32,
    frames: u32,
    rate: f64,
    events: *mut u8,
    position: *mut usize,
    capacity: usize,
) -> bool {
    let engine = unsafe { get(h) };
    let layout = engine.playback.layout();
    let Some(slot) = layout.slot_of(track) else {
        return false;
    };
    if track == layout.first_track() {
        engine.activity.pulse();
        unsafe { engine.playback.begin(frames, rate) };
    }
    let chord = layout.input(crate::layout::Layout::group_of(slot)) == Input::Chord;
    if !chord || position.is_null() {
        return unsafe { engine.playback.render(slot, frames, rate, |_, _, _, _| {}) };
    }
    let mut ump = Ump {
        events,
        capacity,
        end: 0,
        rate,
        ticks: 0,
    };
    let sounds = unsafe {
        engine
            .playback
            .render(slot, frames, rate, |at, pitch, on, velocity| {
                ump.push(at, pitch, on, velocity)
            })
    };
    unsafe { position.write(ump.end) };
    sounds
}

/// Channels past `MAX_CHANNELS` are silenced; null channels are skipped.
unsafe fn mix<T: Sample>(
    engine: &Engine,
    slot: usize,
    out: *const *mut T,
    count: u32,
    frames: u32,
) {
    let frames = frames as usize;
    let mut channels: [&mut [T]; MAX_CHANNELS] = Default::default();
    let mut used = 0;
    for i in 0..count as usize {
        let samples = unsafe { *out.add(i) };
        if samples.is_null() {
            continue;
        }
        if used == MAX_CHANNELS {
            unsafe { samples.write_bytes(0, frames) };
            continue;
        }
        channels[used] = unsafe { std::slice::from_raw_parts_mut(samples, frames) };
        used += 1;
    }
    unsafe { engine.playback.mix(slot, &mut channels[..used], frames) };
}

pub(super) unsafe extern "C" fn after(
    h: *mut c_void,
    track: i32,
    channels: *const *mut c_void,
    count: u32,
    frames: u32,
    wide: bool,
) {
    let engine = unsafe { get(h) };
    let Some(slot) = engine.playback.layout().slot_of(track) else {
        return;
    };
    if !channels.is_null() && count > 0 {
        if wide {
            unsafe { mix::<f64>(engine, slot, channels.cast(), count, frames) };
        } else {
            unsafe { mix::<f32>(engine, slot, channels.cast(), count, frames) };
        }
    }
    unsafe { engine.playback.finish(slot) };
}
