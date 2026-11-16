use crate::evolution::Evolution;
use repl_extension_api::{
    evolution::Api,
    sequence::{Emit, Plan},
};
use std::ffi::c_void;
struct Owned {
    engine: Evolution,
    activity: crate::activity::Activity,
}
unsafe fn get<'a>(ptr: *mut c_void) -> &'a Evolution {
    unsafe { &(*ptr.cast::<Owned>()).engine }
}
unsafe extern "C" fn create(hold: f64, fade: f64) -> *mut c_void {
    let Some(engine) = Evolution::new(hold, fade) else {
        return std::ptr::null_mut();
    };
    let Ok(activity) = crate::activity::Activity::new() else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(Owned { engine, activity })).cast()
}
unsafe extern "C" fn destroy(p: *mut c_void) {
    drop(unsafe { Box::from_raw(p.cast::<Owned>()) });
}
unsafe extern "C" fn pulse(p: *mut c_void) {
    unsafe { (*p.cast::<Owned>()).activity.pulse() };
}
unsafe extern "C" fn spinning(p: *mut c_void) -> bool {
    unsafe { (*p.cast::<Owned>()).activity.spinning() }
}
unsafe extern "C" fn update_priority() {
    crate::thread_priority::update();
}
unsafe extern "C" fn boosted_threads() -> u32 {
    crate::thread_priority::boosted()
}
unsafe extern "C" fn status(p: *mut c_void, s: u32) -> u32 {
    unsafe { get(p).status(s as usize) }
}
unsafe extern "C" fn gain(p: *mut c_void, s: u32) -> f32 {
    unsafe { get(p).gain(s as usize) }
}
unsafe extern "C" fn claim(p: *mut c_void, s: u32) -> bool {
    unsafe { get(p).claim(s as usize) }
}
unsafe extern "C" fn ready(p: *mut c_void, s: u32, ok: bool) {
    unsafe { get(p).ready(s as usize, ok) };
}
unsafe extern "C" fn publish(p: *mut c_void, plan: *const Plan) -> u32 {
    unsafe { get(p).publish(&*plan) }
}
unsafe extern "C" fn pending(p: *mut c_void) -> bool {
    unsafe { get(p).pending() }
}
unsafe extern "C" fn begin(p: *mut c_void, n: u32, r: f64) {
    unsafe { get(p).begin(n, r) };
}
unsafe extern "C" fn render(
    p: *mut c_void,
    s: u32,
    n: u32,
    r: f64,
    emit: Emit,
    user: *mut c_void,
) -> bool {
    unsafe {
        get(p).render(s as usize, n, r, |at, pitch, on, velocity| {
            emit(user, at, pitch, on, velocity)
        })
    }
}
unsafe extern "C" fn mix(p: *mut c_void, s: u32, out: *mut f32, n: u32) {
    unsafe { get(p).mix(s as usize, std::slice::from_raw_parts_mut(out, n as usize)) };
}
unsafe extern "C" fn finish(p: *mut c_void, s: u32) {
    unsafe { get(p).finish(s as usize) };
}
unsafe extern "C" fn mix64(p: *mut c_void, s: u32, out: *mut f64, n: u32) {
    unsafe { get(p).mix64(s as usize, std::slice::from_raw_parts_mut(out, n as usize)) };
}
#[no_mangle]
pub extern "C" fn repl_evolution_api() -> *const Api {
    &Api {
        version: 3,
        create,
        destroy,
        status,
        gain,
        claim,
        ready,
        publish,
        pending,
        begin,
        render,
        mix,
        mix64,
        finish,
        pulse,
        spinning,
        update_priority,
        boosted_threads,
    }
}
