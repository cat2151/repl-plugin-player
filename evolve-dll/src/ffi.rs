//! `repl_evolve_api`: the C ABI of [`Engine`].
mod host;
#[cfg(test)]
mod tests;

use std::{
    collections::VecDeque,
    ffi::{c_char, c_void},
    sync::{Arc, Mutex},
};

use repl_extension_api::{
    evolve::{
        slot_state, Api, HostApi, OutcomeC, SlotStatusC, ERROR_LEN, FORMAT_LEN, PLUGIN_ID_LEN,
        VERSION,
    },
    sequence::{Emit, Plan},
};

use crate::{
    activity::Activity,
    host::PluginKind,
    level::{Sample, MAX_CHANNELS},
    loader::Loader,
    ownership::{Ownership, ENGAGED, LOADING, READY, SLOT_COUNT},
    playback::Playback,
    preload::PreloadOutcome,
};

use host::{text, FfiHost};

/// One evolve session: the loader's workers and the playback side share the
/// slot ownership.
pub struct Engine {
    ownership: Arc<Ownership>,
    playback: Playback,
    loader: Loader,
    /// Outcomes `poll` could not hand out yet.
    unread: Mutex<VecDeque<PreloadOutcome>>,
    /// Pulsed by `begin`, so it spins only while evolve plays.
    activity: Activity,
}

impl Engine {
    fn status(&self, slot: usize) -> SlotStatusC {
        let state = match self.ownership.get(slot) {
            LOADING => slot_state::LOADING,
            READY => slot_state::READY,
            ENGAGED if self.playback.fading_out(slot) => slot_state::RETIRING,
            ENGAGED => slot_state::ACTIVE,
            _ => slot_state::EMPTY,
        };
        let (loudness, peak, trim) = self.playback.measured(slot);
        let mut out = SlotStatusC {
            state,
            gain: self.playback.gain(slot),
            loudness,
            peak,
            trim,
            instance: 0,
            format: [0; FORMAT_LEN],
            plugin_id: [0; PLUGIN_ID_LEN],
        };
        if let Some((kind, handle)) = &self.loader.view().slots[slot] {
            out.instance = *handle;
            copy_text(&mut out.format, &kind.format);
            copy_text(&mut out.plugin_id, &kind.plugin_id);
        }
        out
    }
}

/// Copies `text` NUL-terminated, cut at a character boundary when too long.
fn copy_text(out: &mut [c_char], text: &str) {
    let mut end = text.len().min(out.len() - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    for (dst, src) in out.iter_mut().zip(&text.as_bytes()[..end]) {
        *dst = *src as c_char;
    }
    out[end] = 0;
}

unsafe fn get<'a>(h: *mut c_void) -> &'a Engine {
    unsafe { &*h.cast::<Engine>() }
}

unsafe extern "C" fn create(hold: f64, fade: f64, host: *const HostApi) -> *mut c_void {
    let Some(host) = (unsafe { FfiHost::new(host) }) else {
        return std::ptr::null_mut();
    };
    let ownership = Arc::new(Ownership::default());
    let Some(playback) = Playback::new(hold, fade, Arc::clone(&ownership)) else {
        return std::ptr::null_mut();
    };
    let Ok(activity) = Activity::new() else {
        return std::ptr::null_mut();
    };
    let loader = Loader::spawn(Arc::new(host), Arc::clone(&ownership));
    Box::into_raw(Box::new(Engine {
        ownership,
        playback,
        loader,
        unread: Mutex::default(),
        activity,
    }))
    .cast()
}

unsafe extern "C" fn destroy(h: *mut c_void) {
    drop(unsafe { Box::from_raw(h.cast::<Engine>()) });
}

unsafe extern "C" fn submit(
    h: *mut c_void,
    slot: u32,
    format: *const c_char,
    plugin_id: *const c_char,
    data: *const u8,
    size: usize,
    generation: u64,
) -> u32 {
    let (Some(format), Some(plugin_id), Ok(slot)) = (
        unsafe { text(format) },
        unsafe { text(plugin_id) },
        u8::try_from(slot),
    ) else {
        return 0;
    };
    let state = match (data.is_null(), size) {
        (_, 0) => Vec::new(),
        (true, _) => return 0,
        (false, _) => unsafe { std::slice::from_raw_parts(data, size) }.to_vec(),
    };
    let kind = PluginKind { format, plugin_id };
    unsafe { get(h) }
        .loader
        .submit(slot, kind, state, generation)
}

unsafe extern "C" fn poll(h: *mut c_void, out: *mut OutcomeC, max: u32) -> u32 {
    let engine = unsafe { get(h) };
    let Ok(mut unread) = engine.unread.lock() else {
        return 0;
    };
    unread.extend(engine.loader.poll());
    let count = unread.len().min(max as usize);
    for (i, outcome) in unread.drain(..count).enumerate() {
        let mut c = OutcomeC {
            request_id: outcome.request_id,
            slot: u32::from(outcome.slot),
            elapsed_ms: outcome.elapsed.as_secs_f64() * 1000.0,
            stale: outcome.stale,
            error: [0; ERROR_LEN],
        };
        copy_text(&mut c.error, outcome.error.as_deref().unwrap_or(""));
        unsafe { out.add(i).write(c) };
    }
    count as u32
}

unsafe extern "C" fn bump_generation(h: *mut c_void) -> u64 {
    unsafe { get(h) }.loader.bump_generation()
}

unsafe extern "C" fn status(h: *mut c_void, slot: u32, out: *mut SlotStatusC) -> bool {
    let slot = slot as usize;
    if slot >= SLOT_COUNT || out.is_null() {
        return false;
    }
    unsafe { out.write(get(h).status(slot)) };
    true
}

unsafe extern "C" fn publish(h: *mut c_void, plan: *const Plan) -> u32 {
    match unsafe { plan.as_ref() } {
        Some(plan) => unsafe { get(h) }.playback.publish(plan),
        None => 1,
    }
}

unsafe extern "C" fn pending(h: *mut c_void) -> bool {
    unsafe { get(h) }.playback.pending()
}

unsafe extern "C" fn begin(h: *mut c_void, frames: u32, rate: f64) {
    let engine = unsafe { get(h) };
    engine.activity.pulse();
    unsafe { engine.playback.begin(frames, rate) };
}

unsafe extern "C" fn render(
    h: *mut c_void,
    slot: u32,
    frames: u32,
    rate: f64,
    emit: Emit,
    user: *mut c_void,
) -> bool {
    unsafe {
        get(h)
            .playback
            .render(slot as usize, frames, rate, |at, pitch, on, velocity| {
                emit(user, at, pitch, on, velocity)
            })
    }
}

/// Channels past `MAX_CHANNELS` are silenced; null channels are skipped.
unsafe fn mix_channels<T: Sample>(
    h: *mut c_void,
    slot: u32,
    out: *const *mut T,
    count: u32,
    frames: u32,
) {
    if out.is_null() || slot as usize >= SLOT_COUNT {
        return;
    }
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
    unsafe {
        get(h)
            .playback
            .mix(slot as usize, &mut channels[..used], frames)
    };
}

unsafe extern "C" fn mix(h: *mut c_void, slot: u32, out: *const *mut f32, count: u32, frames: u32) {
    unsafe { mix_channels(h, slot, out, count, frames) };
}

unsafe extern "C" fn mix64(
    h: *mut c_void,
    slot: u32,
    out: *const *mut f64,
    count: u32,
    frames: u32,
) {
    unsafe { mix_channels(h, slot, out, count, frames) };
}

unsafe extern "C" fn finish(h: *mut c_void, slot: u32) {
    unsafe { get(h).playback.finish(slot as usize) };
}

#[no_mangle]
pub extern "C" fn repl_evolve_api() -> *const Api {
    &Api {
        version: VERSION,
        api_size: size_of::<Api>(),
        host_size: size_of::<HostApi>(),
        outcome_size: size_of::<OutcomeC>(),
        status_size: size_of::<SlotStatusC>(),
        plan_size: size_of::<Plan>(),
        create,
        destroy,
        submit,
        poll,
        bump_generation,
        status,
        publish,
        pending,
        begin,
        render,
        mix,
        mix64,
        finish,
    }
}
