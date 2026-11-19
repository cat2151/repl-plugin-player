//! Drives the exported ABI with a C-shaped host whose callbacks arrive from
//! other threads.
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    sync::Mutex,
    time::{Duration, Instant},
};

use super::*;

#[derive(Default)]
struct CHost {
    next: Mutex<u64>,
    alive: Mutex<HashMap<u64, String>>,
    attached: Mutex<HashMap<u32, u64>>,
    states: Mutex<Vec<Vec<u8>>>,
}

unsafe fn host<'a>(ctx: *mut c_void) -> &'a CHost {
    unsafe { &*ctx.cast::<CHost>() }
}

/// Moves a raw pointer to the callback thread.
struct User(*mut c_void);
unsafe impl Send for User {}

unsafe extern "C" fn create_instance(
    ctx: *mut c_void,
    _format: *const c_char,
    plugin_id: *const c_char,
    done: repl_extension_api::evolve2::CreateDone,
    user: *mut c_void,
) {
    let h = unsafe { host(ctx) };
    let id = unsafe { CStr::from_ptr(plugin_id) }
        .to_string_lossy()
        .into_owned();
    let user = User(user);
    if id == "broken" {
        std::thread::spawn(move || {
            let user = user;
            unsafe { done(user.0, 0, c"cannot create".as_ptr()) }
        });
        return;
    }
    let mut next = h.next.lock().unwrap();
    *next += 1;
    let handle = *next;
    h.alive.lock().unwrap().insert(handle, id);
    std::thread::spawn(move || {
        let user = user;
        unsafe { done(user.0, handle, std::ptr::null()) }
    });
}

unsafe extern "C" fn destroy_instance(ctx: *mut c_void, handle: u64) {
    unsafe { host(ctx) }.alive.lock().unwrap().remove(&handle);
}

unsafe extern "C" fn load_state(
    ctx: *mut c_void,
    _handle: u64,
    data: *const u8,
    size: usize,
    done: repl_extension_api::evolve2::LoadDone,
    user: *mut c_void,
) {
    let state = unsafe { std::slice::from_raw_parts(data, size) }.to_vec();
    unsafe { host(ctx) }.states.lock().unwrap().push(state);
    let user = User(user);
    std::thread::spawn(move || {
        let user = user;
        unsafe { done(user.0, true, std::ptr::null()) }
    });
}

unsafe extern "C" fn attach_to_slot(ctx: *mut c_void, slot: u32, handle: u64) -> bool {
    unsafe { host(ctx) }
        .attached
        .lock()
        .unwrap()
        .insert(slot, handle);
    true
}

unsafe extern "C" fn detach_from_slot(ctx: *mut c_void, slot: u32) -> u64 {
    let removed = unsafe { host(ctx) }.attached.lock().unwrap().remove(&slot);
    removed.unwrap_or(0)
}

fn host_api(ctx: &CHost) -> HostApi {
    HostApi {
        size: size_of::<HostApi>(),
        ctx: std::ptr::from_ref(ctx).cast_mut().cast(),
        create_instance,
        destroy_instance,
        load_state,
        attach_to_slot,
        detach_from_slot,
    }
}

fn api() -> &'static Api {
    unsafe { &*repl_evolve2_api() }
}

fn wait_outcome(h: *mut c_void) -> OutcomeC {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut out = [OutcomeC {
        request_id: 0,
        slot: 0,
        elapsed_ms: 0.0,
        stale: false,
        error: [0; ERROR_LEN],
    }; 1];
    while unsafe { (api().poll)(h, out.as_mut_ptr(), 1) } == 0 {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
    out[0]
}

fn c_text(chars: &[c_char]) -> String {
    unsafe { CStr::from_ptr(chars.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn status(h: *mut c_void, slot: u32) -> SlotStatusC {
    let mut out = std::mem::MaybeUninit::uninit();
    assert!(unsafe { (api().status)(h, slot, out.as_mut_ptr()) });
    unsafe { out.assume_init() }
}

#[test]
fn the_table_reports_its_sizes() {
    let api = api();
    assert_eq!(api.version, VERSION);
    assert_eq!(api.api_size, size_of::<Api>());
    assert_eq!(api.host_size, size_of::<HostApi>());
    assert_eq!(api.plan_size, size_of::<Plan>());
}

#[test]
fn invalid_arguments_are_rejected() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    unsafe {
        assert!((api().create)(5.0, 1.0, std::ptr::null()).is_null());
        assert!((api().create)(4.0, 1.0, &host).is_null(), "hold below 5 s");
        let small = HostApi { size: 8, ..host };
        assert!((api().create)(5.0, 1.0, &small).is_null());
        let h = (api().create)(5.0, 1.0, &host);
        assert!(!h.is_null());
        let id = c"x".as_ptr();
        assert_eq!(
            (api().submit)(h, 0, std::ptr::null(), id, std::ptr::null(), 0, 0),
            0
        );
        assert_eq!((api().submit)(h, 2, id, id, std::ptr::null(), 0, 0), 0);
        assert_eq!((api().submit)(h, 0, id, id, std::ptr::null(), 3, 0), 0);
        let mut out = std::mem::MaybeUninit::uninit();
        assert!(!(api().status)(h, 2, out.as_mut_ptr()));
        (api().destroy)(h);
    }
}

#[test]
fn a_request_settles_through_poll_and_shows_in_status() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    let h = unsafe { (api().create)(5.0, 1.0, &host) };
    let state = [7u8, 8, 9];
    let id = unsafe {
        (api().submit)(
            h,
            1,
            c"CLAP".as_ptr(),
            c"org.example.synth".as_ptr(),
            state.as_ptr(),
            state.len(),
            0,
        )
    };
    assert_ne!(id, 0);
    let outcome = wait_outcome(h);
    assert_eq!((outcome.request_id, outcome.slot), (id, 1));
    assert!(!outcome.stale && c_text(&outcome.error).is_empty());
    assert!(outcome.elapsed_ms >= 0.0);
    assert_eq!(ctx.states.lock().unwrap().as_slice(), [state.to_vec()]);

    let slot = status(h, 1);
    assert_eq!(slot.state, slot_state::READY);
    assert_eq!(ctx.attached.lock().unwrap().get(&1), Some(&slot.instance));
    assert_eq!(c_text(&slot.format), "CLAP");
    assert_eq!(c_text(&slot.plugin_id), "org.example.synth");
    assert_eq!(status(h, 0).state, slot_state::EMPTY);
    assert_eq!(status(h, 0).instance, 0);

    unsafe { (api().destroy)(h) };
    assert!(ctx.alive.lock().unwrap().is_empty(), "destroy frees all");
}

#[test]
fn errors_and_stale_outcomes_cross_the_boundary() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    let h = unsafe { (api().create)(5.0, 1.0, &host) };
    let submit = |plugin: &CStr, generation| unsafe {
        (api().submit)(
            h,
            0,
            c"CLAP".as_ptr(),
            plugin.as_ptr(),
            std::ptr::null(),
            0,
            generation,
        )
    };
    submit(c"broken", 0);
    assert_eq!(c_text(&wait_outcome(h).error), "cannot create");
    assert_eq!(unsafe { (api().bump_generation)(h) }, 1);
    submit(c"a", 0);
    assert!(wait_outcome(h).stale);
    unsafe { (api().destroy)(h) };
}

#[test]
fn long_text_is_cut_at_a_character_boundary() {
    let mut out = [1 as c_char; 4];
    copy_text(&mut out, "aあ");
    assert_eq!(c_text(&out), "a");
    copy_text(&mut out, "abcdef");
    assert_eq!(c_text(&out), "abc");
}
