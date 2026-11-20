//! evolve ABI. Requests return a request id at once and are settled by `poll`;
//! the host side ([`HostApi`]) reports instance creation and state loading
//! through `done` callbacks instead of returning.
//! Slot `2 * group + i` plays track `i` of the groups given to `create`.
use super::sequence::Plan;
use std::ffi::{c_char, c_void};

pub const VERSION: u32 = 3;
/// `submit`'s generation for "the generation `bump_generation` last returned".
pub const CURRENT_GENERATION: u64 = u64::MAX;

/// `error` is null on success; `handle` is never 0 on success. `instance` is
/// the host's id of the plugin instance, shown by `status`.
pub type CreateDone =
    unsafe extern "C" fn(user: *mut c_void, handle: u64, instance: i32, error: *const c_char);
/// `error` may be null when `ok` is true.
pub type LoadDone = unsafe extern "C" fn(user: *mut c_void, ok: bool, error: *const c_char);

/// Implemented by the plugin host. Every `done` must be called exactly once,
/// from any thread. `load_state` copies `data` before returning.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostApi {
    pub size: usize,
    pub ctx: *mut c_void,
    /// `(ctx, format, plugin_id, done, user)`; strings are NUL-terminated UTF-8.
    pub create_instance:
        unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, CreateDone, *mut c_void),
    pub destroy_instance: unsafe extern "C" fn(*mut c_void, u64),
    /// `(ctx, handle, data, size, done, user)`; `data` is a `patchJob` patch.
    pub load_state: unsafe extern "C" fn(*mut c_void, u64, *const u8, usize, LoadDone, *mut c_void),
    pub attach_to_slot: unsafe extern "C" fn(*mut c_void, u32, u64) -> bool,
    /// Returns the detached handle, or 0 when the slot had none.
    pub detach_from_slot: unsafe extern "C" fn(*mut c_void, u32) -> u64,
}

/// Tracks that crossfade into each other. `input` is `"chord"` (notes from
/// evolve's chord clock, loudness-trimmed) or `"timeline"` (the events the
/// track already receives, untrimmed); NUL-terminated.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GroupC {
    pub tracks: *const i32,
    pub track_count: u32,
    pub input: *const c_char,
}

/// Values of `state` in `status`.
pub mod slot_state {
    /// Neither loading nor sounding; the instance, if any, is silent.
    pub const EMPTY: u32 = 0;
    pub const LOADING: u32 = 1;
    pub const READY: u32 = 2;
    pub const ACTIVE: u32 = 3;
    /// Sounding and fading out.
    pub const RETIRING: u32 = 4;
}

#[repr(C)]
pub struct Api {
    pub version: u32,
    pub api_size: usize,
    pub host_size: usize,
    pub group_size: usize,
    pub plan_size: usize,
    /// `(groups, group_count, hold_sec, fade_sec, host)`; null when a group
    /// has other than two tracks, a track repeats, an input is unknown, or
    /// hold or fade is invalid.
    pub create: unsafe extern "C" fn(*const GroupC, u32, f64, f64, *const HostApi) -> *mut c_void,
    /// Waits for the workers and destroys every instance. Audio callbacks
    /// must be excluded first.
    pub destroy: unsafe extern "C" fn(*mut c_void),
    pub slot_count: unsafe extern "C" fn(*mut c_void) -> u32,
    /// The slot playing `track`, or -1. Lock-free.
    pub slot_of: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    /// `(h, slot, format, plugin_id, file, generation)` → request id; 0 when
    /// rejected or the prepared patch `file` (UTF-8 path) cannot be read.
    pub submit: unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_char,
        *const c_char,
        *const c_char,
        u64,
    ) -> u32,
    /// Every settled request as a JSON array of `{requestId, slot, elapsedMs,
    /// stale, error}`. Valid until the next `poll`.
    pub poll: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    pub bump_generation: unsafe extern "C" fn(*mut c_void) -> u64,
    /// Every slot as a JSON array of `{state, gain, loudness, peak, trim,
    /// instance, instanceId, format, pluginId}`; unmeasured values are null.
    /// Valid until the next `status`.
    pub status: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    /// `(h, plan, tracks, track_count)`: the chord pattern of the `"chord"`
    /// group with exactly these tracks; `plan.track` is its first track.
    /// 0: accepted, 1: invalid, 2: queue full, 3: no such group.
    pub publish: unsafe extern "C" fn(*mut c_void, *const Plan, *const i32, u32) -> u32,
    pub pending: unsafe extern "C" fn(*mut c_void) -> bool,
    /// `(h, track, frames, rate, events, position, capacity)` before the
    /// track processes: the first track of the groups begins the block. A
    /// `"chord"` slot's note events replace the UMP words in `events` from 0
    /// and `*position` becomes their end; a `"timeline"` slot leaves them.
    /// Returns whether the slot sounds this block.
    pub before:
        unsafe extern "C" fn(*mut c_void, i32, u32, f64, *mut u8, *mut usize, usize) -> bool,
    /// `(h, track, channels, channel_count, frames, wide)`: every output
    /// channel in one call (the limiter links them), `f64` when `wide`.
    /// Called for every track `before` was, with no channels when silent.
    pub after: unsafe extern "C" fn(*mut c_void, i32, *const *mut c_void, u32, u32, bool),
}
