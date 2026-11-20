//! `repl_evolve_api`: the C ABI of [`Engine`].
mod audio;
mod host;
mod json;
#[cfg(test)]
mod tests;

use std::{
    ffi::{c_char, c_void, CString},
    sync::{Arc, Mutex},
};

use repl_extension_api::{
    evolve::{slot_state, Api, GroupC, HostApi, CURRENT_GENERATION, VERSION},
    sequence::Plan,
};

use crate::{
    activity::Activity,
    host::PluginKind,
    layout::{Input, Layout, GROUP_TRACKS},
    loader::Loader,
    ownership::{Ownership, ENGAGED, LOADING, READY},
    playback::Playback,
};

use host::{text, FfiHost, InstanceIds};
use json::{array, Object};

/// One evolve session: the loader's workers and the playback side share the
/// slot ownership.
pub struct Engine {
    ownership: Arc<Ownership>,
    playback: Playback,
    loader: Loader,
    /// Pulsed when a block begins, so it spins only while evolve plays.
    activity: Activity,
    ids: InstanceIds,
    /// The texts `status` and `poll` returned last.
    status_text: Mutex<CString>,
    poll_text: Mutex<CString>,
}

impl Engine {
    fn status_json(&self) -> String {
        let view = self.loader.view();
        let ids = self.ids.lock().map(|ids| ids.clone()).unwrap_or_default();
        array(0..self.ownership.len(), |out, slot| {
            let state = match self.ownership.get(slot) {
                LOADING => slot_state::LOADING,
                READY => slot_state::READY,
                ENGAGED if self.playback.fading_out(slot) => slot_state::RETIRING,
                ENGAGED => slot_state::ACTIVE,
                _ => slot_state::EMPTY,
            };
            let (loudness, peak, trim) = self.playback.measured(slot);
            let (kind, handle) = match view.slots.get(slot).and_then(Option::as_ref) {
                Some((kind, handle)) => (Some(kind), *handle),
                None => (None, 0),
            };
            let id = ids.get(&handle).copied().unwrap_or(-1);
            Object::open(out)
                .number("state", f64::from(state))
                .number("gain", f64::from(self.playback.gain(slot)))
                .number("loudness", f64::from(loudness))
                .number("peak", f64::from(peak))
                .number("trim", f64::from(trim))
                .number("instance", handle as f64)
                .number("instanceId", f64::from(id))
                .string("format", kind.map_or("", |k| &k.format))
                .string("pluginId", kind.map_or("", |k| &k.plugin_id))
                .close();
        })
    }

    fn poll_json(&self) -> String {
        array(self.loader.poll(), |out, outcome| {
            Object::open(out)
                .number("requestId", f64::from(outcome.request_id))
                .number("slot", f64::from(outcome.slot))
                .number("elapsedMs", outcome.elapsed.as_secs_f64() * 1000.0)
                .boolean("stale", outcome.stale)
                .string("error", outcome.error.as_deref().unwrap_or(""))
                .close();
        })
    }
}

/// Keeps `json` in `cell` until the next call and returns it.
fn hand_out(cell: &Mutex<CString>, json: String) -> *const c_char {
    let Ok(mut kept) = cell.lock() else {
        return c"[]".as_ptr();
    };
    // The writer escapes NUL, so this does not fail.
    *kept = CString::new(json).unwrap_or_default();
    kept.as_ptr()
}

unsafe fn get<'a>(h: *mut c_void) -> &'a Engine {
    unsafe { &*h.cast::<Engine>() }
}

/// # Safety
/// `groups` points to `count` readable groups, each with readable tracks.
unsafe fn layout(groups: *const GroupC, count: u32) -> Option<Layout> {
    if groups.is_null() {
        return None;
    }
    let mut parsed = Vec::new();
    for group in unsafe { std::slice::from_raw_parts(groups, count as usize) } {
        if group.track_count as usize != GROUP_TRACKS || group.tracks.is_null() {
            return None;
        }
        let tracks = unsafe { std::slice::from_raw_parts(group.tracks, GROUP_TRACKS) };
        let input = Input::parse(&unsafe { text(group.input) }?)?;
        parsed.push(([tracks[0], tracks[1]], input));
    }
    Layout::new(&parsed)
}

unsafe extern "C" fn create(
    groups: *const GroupC,
    count: u32,
    hold: f64,
    fade: f64,
    host: *const HostApi,
) -> *mut c_void {
    let Some(layout) = (unsafe { layout(groups, count) }) else {
        return std::ptr::null_mut();
    };
    let ids = InstanceIds::default();
    let Some(host) = (unsafe { FfiHost::new(host, Arc::clone(&ids)) }) else {
        return std::ptr::null_mut();
    };
    let ownership = Arc::new(Ownership::new(layout.slot_count()));
    let Some(playback) = Playback::new(layout, hold, fade, Arc::clone(&ownership)) else {
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
        activity,
        ids,
        status_text: Mutex::default(),
        poll_text: Mutex::default(),
    }))
    .cast()
}

unsafe extern "C" fn destroy(h: *mut c_void) {
    drop(unsafe { Box::from_raw(h.cast::<Engine>()) });
}

unsafe extern "C" fn slot_count(h: *mut c_void) -> u32 {
    unsafe { get(h) }.ownership.len() as u32
}

unsafe extern "C" fn slot_of(h: *mut c_void, track: i32) -> i32 {
    let slot = unsafe { get(h) }.playback.layout().slot_of(track);
    slot.map_or(-1, |slot| slot as i32)
}

unsafe extern "C" fn submit(
    h: *mut c_void,
    slot: u32,
    format: *const c_char,
    plugin_id: *const c_char,
    file: *const c_char,
    generation: u64,
) -> u32 {
    let (Some(format), Some(plugin_id), Some(file), Ok(slot)) = (
        unsafe { text(format) },
        unsafe { text(plugin_id) },
        unsafe { text(file) },
        u8::try_from(slot),
    ) else {
        return 0;
    };
    let Ok(state) = std::fs::read(file) else {
        return 0;
    };
    let loader = &unsafe { get(h) }.loader;
    let generation = match generation {
        CURRENT_GENERATION => loader.generation(),
        generation => generation,
    };
    loader.submit(slot, PluginKind { format, plugin_id }, state, generation)
}

unsafe extern "C" fn poll(h: *mut c_void) -> *const c_char {
    let engine = unsafe { get(h) };
    hand_out(&engine.poll_text, engine.poll_json())
}

unsafe extern "C" fn bump_generation(h: *mut c_void) -> u64 {
    unsafe { get(h) }.loader.bump_generation()
}

unsafe extern "C" fn status(h: *mut c_void) -> *const c_char {
    let engine = unsafe { get(h) };
    hand_out(&engine.status_text, engine.status_json())
}

unsafe extern "C" fn publish(
    h: *mut c_void,
    plan: *const Plan,
    tracks: *const i32,
    count: u32,
) -> u32 {
    let (Some(plan), false) = (unsafe { plan.as_ref() }, tracks.is_null()) else {
        return 1;
    };
    let tracks = unsafe { std::slice::from_raw_parts(tracks, count as usize) };
    unsafe { get(h) }.playback.publish(plan, tracks)
}

unsafe extern "C" fn pending(h: *mut c_void) -> bool {
    unsafe { get(h) }.playback.pending()
}

#[no_mangle]
pub extern "C" fn repl_evolve_api() -> *const Api {
    &Api {
        version: VERSION,
        api_size: size_of::<Api>(),
        host_size: size_of::<HostApi>(),
        group_size: size_of::<GroupC>(),
        plan_size: size_of::<Plan>(),
        create,
        destroy,
        slot_count,
        slot_of,
        submit,
        poll,
        bump_generation,
        status,
        publish,
        pending,
        before: audio::before,
        after: audio::after,
    }
}
