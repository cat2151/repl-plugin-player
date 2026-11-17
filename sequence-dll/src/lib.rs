//! Sample-clock sequencing. The control producer and serialized audio consumer
//! share bounded storage; render never allocates, locks, or calls JavaScript.
mod activity;
mod clock;
mod evolution;
mod evolution_ffi;
mod note_count;
#[cfg(test)]
mod tests;
mod thread_priority;
mod voicing;

use clock::Clock;
use repl_extension_api::sequence::{Api, Emit, Plan, Step};
use std::ffi::c_void;

#[no_mangle]
pub fn repl_sequence_choose(
    candidates: &[Vec<Vec<u8>>],
    previous: &[Vec<u8>],
    max_leap: u8,
    pin: Option<usize>,
) -> Option<Vec<usize>> {
    voicing::choose(candidates, previous, max_leap, pin)
}

unsafe extern "C" fn create() -> *mut c_void {
    Box::into_raw(Box::<Clock>::default()).cast()
}
unsafe extern "C" fn destroy(ptr: *mut c_void) {
    drop(unsafe { Box::from_raw(ptr.cast::<Clock>()) });
}
unsafe extern "C" fn publish(ptr: *mut c_void, plan: *const Plan) -> u32 {
    unsafe { (&*ptr.cast::<Clock>()).publish(&*plan) }
}
unsafe extern "C" fn reset(ptr: *mut c_void) {
    unsafe { (&*ptr.cast::<Clock>()).reset() };
}
unsafe extern "C" fn pending(ptr: *mut c_void) -> bool {
    unsafe { (&*ptr.cast::<Clock>()).pending() }
}
unsafe extern "C" fn rewind(ptr: *mut c_void) {
    unsafe { (&*ptr.cast::<Clock>()).rewind() };
}
unsafe extern "C" fn held(ptr: *mut c_void, out: *mut Step) {
    unsafe { *out = (&*ptr.cast::<Clock>()).held() };
}
unsafe extern "C" fn release(ptr: *mut c_void) {
    unsafe { (&*ptr.cast::<Clock>()).release() };
}
unsafe extern "C" fn render(
    ptr: *mut c_void,
    track: i32,
    frames: u32,
    rate: f64,
    enabled: bool,
    emit: Emit,
    user: *mut c_void,
) {
    unsafe {
        (&*ptr.cast::<Clock>()).render_enabled(
            track,
            frames,
            rate,
            enabled,
            |offset, pitch, on, velocity| emit(user, offset, pitch, on, velocity),
        )
    };
}

#[no_mangle]
pub extern "C" fn repl_sequence_api() -> *const Api {
    &Api {
        version: 3,
        plan_size: size_of::<Plan>(),
        step_size: size_of::<Step>(),
        create,
        destroy,
        publish,
        pending,
        reset,
        rewind,
        held,
        render,
        release,
    }
}
