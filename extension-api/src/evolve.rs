//! evolve ABI. Requests return a request id at once and are settled by `poll`;
//! the host side ([`HostApi`]) reports instance creation and state loading
//! through `done` callbacks instead of returning.
use super::sequence::{Emit, Plan};
use std::ffi::{c_char, c_void};

pub const VERSION: u32 = 2;
pub const ERROR_LEN: usize = 256;
pub const FORMAT_LEN: usize = 16;
pub const PLUGIN_ID_LEN: usize = 256;

/// `error` is null on success; `handle` is never 0 on success.
pub type CreateDone = unsafe extern "C" fn(user: *mut c_void, handle: u64, error: *const c_char);
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
    /// `(ctx, handle, data, size, done, user)`; `data` is a patch prepared for
    /// the host (`patchJob`), which the host puts into the instance.
    pub load_state: unsafe extern "C" fn(*mut c_void, u64, *const u8, usize, LoadDone, *mut c_void),
    pub attach_to_slot: unsafe extern "C" fn(*mut c_void, u32, u64) -> bool,
    /// Returns the detached handle, or 0 when the slot had none.
    pub detach_from_slot: unsafe extern "C" fn(*mut c_void, u32) -> u64,
}

/// One settled request. `error` is NUL-terminated and empty on success.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OutcomeC {
    pub request_id: u32,
    pub slot: u32,
    pub elapsed_ms: f64,
    pub stale: bool,
    pub error: [c_char; ERROR_LEN],
}

/// Values of [`SlotStatusC::state`].
pub mod slot_state {
    /// Neither loading nor sounding; the instance, if any, is silent.
    pub const EMPTY: u32 = 0;
    pub const LOADING: u32 = 1;
    /// Loaded and waiting for playback to take it.
    pub const READY: u32 = 2;
    pub const ACTIVE: u32 = 3;
    /// Sounding and fading out.
    pub const RETIRING: u32 = 4;
}

/// `instance` is 0 and the names are empty when the slot has no instance.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlotStatusC {
    pub state: u32,
    pub gain: f32,
    /// LUFS of the slot's patch, measured before it was heard; NaN until then.
    pub loudness: f32,
    /// Largest untrimmed sample while measuring, in dBFS; NaN until measured.
    pub peak: f32,
    /// Linear gain that brings `loudness` to the target; NaN until measured.
    pub trim: f32,
    pub instance: u64,
    pub format: [c_char; FORMAT_LEN],
    pub plugin_id: [c_char; PLUGIN_ID_LEN],
}

#[repr(C)]
pub struct Api {
    pub version: u32,
    pub api_size: usize,
    pub host_size: usize,
    pub outcome_size: usize,
    pub status_size: usize,
    pub plan_size: usize,
    /// `(hold_sec, fade_sec, host)`; null when the arguments are invalid.
    pub create: unsafe extern "C" fn(f64, f64, *const HostApi) -> *mut c_void,
    /// Waits for the workers and destroys every instance. Audio callbacks
    /// must be excluded first.
    pub destroy: unsafe extern "C" fn(*mut c_void),
    /// `(h, slot, format, plugin_id, data, size, generation)` → request id; 0 is rejected.
    pub submit: unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_char,
        *const c_char,
        *const u8,
        usize,
        u64,
    ) -> u32,
    /// Writes up to `max` outcomes and returns how many; the rest stay queued.
    pub poll: unsafe extern "C" fn(*mut c_void, *mut OutcomeC, u32) -> u32,
    /// Returns the new generation.
    pub bump_generation: unsafe extern "C" fn(*mut c_void) -> u64,
    /// `false` when the slot does not exist.
    pub status: unsafe extern "C" fn(*mut c_void, u32, *mut SlotStatusC) -> bool,
    /// 0: accepted, 1: invalid, 2: queue full.
    pub publish: unsafe extern "C" fn(*mut c_void, *const Plan) -> u32,
    pub pending: unsafe extern "C" fn(*mut c_void) -> bool,
    pub begin: unsafe extern "C" fn(*mut c_void, u32, f64),
    pub render: unsafe extern "C" fn(*mut c_void, u32, u32, f64, Emit, *mut c_void) -> bool,
    /// `(h, slot, channels, channel_count, frames)`: every output channel of
    /// the slot in one call, so the limiter links them.
    pub mix: unsafe extern "C" fn(*mut c_void, u32, *const *mut f32, u32, u32),
    pub mix64: unsafe extern "C" fn(*mut c_void, u32, *const *mut f64, u32, u32),
    pub finish: unsafe extern "C" fn(*mut c_void, u32),
}
