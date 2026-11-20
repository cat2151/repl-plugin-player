//! Drives the exported ABI with a C-shaped host whose callbacks arrive from
//! other threads.
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use repl_extension_api::sequence::Step;

use super::*;

/// The host's instance id is the handle plus this.
const INSTANCE_ID_OFFSET: u64 = 100;

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
    done: repl_extension_api::evolve::CreateDone,
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
            unsafe { done(user.0, 0, -1, c"cannot create".as_ptr()) }
        });
        return;
    }
    let mut next = h.next.lock().unwrap();
    *next += 1;
    let handle = *next;
    h.alive.lock().unwrap().insert(handle, id);
    std::thread::spawn(move || {
        let user = user;
        let instance = (handle + INSTANCE_ID_OFFSET) as i32;
        unsafe { done(user.0, handle, instance, std::ptr::null()) }
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
    done: repl_extension_api::evolve::LoadDone,
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
    unsafe { &*repl_evolve_api() }
}

/// Groups as the adapter passes them; the arrays outlive `create`.
struct Groups {
    tracks: Vec<[i32; 2]>,
    inputs: Vec<&'static CStr>,
}

impl Groups {
    fn new(groups: &[([i32; 2], &'static CStr)]) -> Self {
        Self {
            tracks: groups.iter().map(|(t, _)| *t).collect(),
            inputs: groups.iter().map(|(_, i)| *i).collect(),
        }
    }

    fn c(&self) -> Vec<GroupC> {
        self.tracks
            .iter()
            .zip(&self.inputs)
            .map(|(tracks, input)| GroupC {
                tracks: tracks.as_ptr(),
                track_count: 2,
                input: input.as_ptr(),
            })
            .collect()
    }

    fn create(&self, hold: f64, host: &HostApi) -> *mut c_void {
        let c = self.c();
        unsafe { (api().create)(c.as_ptr(), c.len() as u32, hold, 1.0, host) }
    }
}

fn pad() -> Groups {
    Groups::new(&[([0, 1], c"chord")])
}

/// A prepared patch file, removed on drop.
struct StateFile(PathBuf, std::ffi::CString);

impl StateFile {
    fn new(name: &str, bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!("evolve-ffi-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let text = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        Self(path, text)
    }
}

impl Drop for StateFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn submit(h: *mut c_void, slot: u32, plugin: &CStr, file: &StateFile, generation: u64) -> u32 {
    unsafe {
        (api().submit)(
            h,
            slot,
            c"CLAP".as_ptr(),
            plugin.as_ptr(),
            file.1.as_ptr(),
            generation,
        )
    }
}

fn text_of(ptr: *const c_char) -> String {
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// The objects of a flat JSON array, as text.
fn objects(json: &str) -> Vec<String> {
    let inner = json.trim_start_matches('[').trim_end_matches(']');
    if inner.is_empty() {
        return Vec::new();
    }
    inner.split("},{").map(str::to_string).collect()
}

/// The raw value of `key` in one object of [`objects`].
fn field(object: &str, key: &str) -> String {
    let start = object.find(&format!("\"{key}\":")).unwrap() + key.len() + 3;
    let rest = &object[start..];
    let end = if let Some(quoted) = rest.strip_prefix('"') {
        quoted.find('"').unwrap() + 2
    } else {
        rest.find([',', '}']).unwrap_or(rest.len())
    };
    rest[..end].to_string()
}

fn wait_outcome(h: *mut c_void) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut settled = objects(&text_of(unsafe { (api().poll)(h) }));
        if !settled.is_empty() {
            assert_eq!(settled.len(), 1);
            return settled.remove(0);
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn status(h: *mut c_void) -> Vec<String> {
    objects(&text_of(unsafe { (api().status)(h) }))
}

#[test]
fn the_table_reports_its_sizes() {
    let api = api();
    assert_eq!(api.version, VERSION);
    assert_eq!(api.api_size, size_of::<Api>());
    assert_eq!(api.host_size, size_of::<HostApi>());
    assert_eq!(api.group_size, size_of::<GroupC>());
    assert_eq!(api.plan_size, size_of::<Plan>());
}

#[test]
fn invalid_layouts_and_arguments_are_rejected() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    let rejected = |groups: &Groups| groups.create(5.0, &host).is_null();
    assert!(rejected(&Groups::new(&[])));
    assert!(rejected(&Groups::new(&[([0, 0], c"chord")])));
    assert!(rejected(&Groups::new(&[
        ([0, 1], c"chord"),
        ([1, 2], c"timeline")
    ])));
    assert!(rejected(&Groups::new(&[([0, 1], c"clip")])));
    assert!(pad().create(4.0, &host).is_null(), "hold below 5 s");
    let three = [0, 1, 2];
    let group = GroupC {
        tracks: three.as_ptr(),
        track_count: 3,
        input: c"chord".as_ptr(),
    };
    assert!(unsafe { (api().create)(&group, 1, 5.0, 1.0, &host) }.is_null());
    let small = HostApi { size: 8, ..host };
    assert!(pad().create(5.0, &small).is_null());
    assert!(unsafe { (api().create)(std::ptr::null(), 0, 5.0, 1.0, &host) }.is_null());

    let h = pad().create(5.0, &host);
    assert!(!h.is_null());
    let file = StateFile::new("rejected", &[1]);
    let id = c"x".as_ptr();
    unsafe {
        let null = std::ptr::null();
        assert_eq!((api().submit)(h, 0, null, id, file.1.as_ptr(), 0), 0);
        assert_eq!((api().submit)(h, 2, id, id, file.1.as_ptr(), 0), 0);
        assert_eq!((api().submit)(h, 0, id, id, c"no such file".as_ptr(), 0), 0);
        assert_eq!((api().slot_count)(h), 2);
        (api().destroy)(h);
    }
}

#[test]
fn a_request_settles_through_poll_and_shows_in_status() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    let h = pad().create(5.0, &host);
    let state = [7u8, 8, 9];
    let file = StateFile::new("settles", &state);
    let id = submit(h, 1, c"org.example.synth", &file, CURRENT_GENERATION);
    assert_ne!(id, 0);
    let outcome = wait_outcome(h);
    assert_eq!(field(&outcome, "requestId"), id.to_string());
    assert_eq!(field(&outcome, "slot"), "1");
    assert_eq!(field(&outcome, "stale"), "false");
    assert_eq!(field(&outcome, "error"), "\"\"");
    assert!(field(&outcome, "elapsedMs").parse::<f64>().unwrap() >= 0.0);
    assert_eq!(ctx.states.lock().unwrap().as_slice(), [state.to_vec()]);

    let slots = status(h);
    assert_eq!(slots.len(), 2);
    assert_eq!(field(&slots[1], "state"), slot_state::READY.to_string());
    let handle = *ctx.attached.lock().unwrap().get(&1).unwrap();
    assert_eq!(field(&slots[1], "instance"), handle.to_string());
    let instance = handle + INSTANCE_ID_OFFSET;
    assert_eq!(field(&slots[1], "instanceId"), instance.to_string());
    assert_eq!(field(&slots[1], "format"), "\"CLAP\"");
    assert_eq!(field(&slots[1], "pluginId"), "\"org.example.synth\"");
    assert_eq!(field(&slots[1], "trim"), "null");
    assert_eq!(field(&slots[0], "state"), slot_state::EMPTY.to_string());
    assert_eq!(field(&slots[0], "instance"), "0");
    assert_eq!(field(&slots[0], "instanceId"), "-1");

    unsafe { (api().destroy)(h) };
    assert!(ctx.alive.lock().unwrap().is_empty(), "destroy frees all");
}

#[test]
fn errors_and_stale_outcomes_cross_the_boundary() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    let h = pad().create(5.0, &host);
    let file = StateFile::new("stale", &[]);
    submit(h, 0, c"broken", &file, CURRENT_GENERATION);
    assert_eq!(field(&wait_outcome(h), "error"), "\"cannot create\"");
    assert_eq!(unsafe { (api().bump_generation)(h) }, 1);
    submit(h, 0, c"a", &file, 0);
    assert_eq!(field(&wait_outcome(h), "stale"), "true");
    submit(h, 0, c"a", &file, CURRENT_GENERATION);
    assert_eq!(field(&wait_outcome(h), "stale"), "false");
    unsafe { (api().destroy)(h) };
}

fn plan(track: i32) -> Plan {
    let mut plan = Plan {
        count: 1,
        track,
        seconds: 1.0,
        gate: 1.0,
        velocity: 20000,
        ..Default::default()
    };
    plan.steps[0] = Step {
        notes: [60, 64, 67, 0, 0, 0, 0, 0],
        count: 3,
    };
    plan
}

/// Words of a note on at offset 0 in the clockstamp-plus-note form.
fn note_on(pitch: u8, velocity: u16) -> [u32; 3] {
    [
        0x0020_0000,
        0x4090_0000 | u32::from(pitch) << 8,
        u32::from(velocity) << 16,
    ]
}

#[test]
fn a_chord_slot_rewrites_its_events_and_a_timeline_slot_keeps_them() {
    let ctx = CHost::default();
    let host = host_api(&ctx);
    // The timeline group holds the first track, so it begins the block.
    let groups = Groups::new(&[([3, 5], c"chord"), ([1, 2], c"timeline")]);
    let h = groups.create(5.0, &host);
    let file = StateFile::new("events", &[1]);
    unsafe {
        assert_eq!((api().slot_count)(h), 4);
        assert_eq!(
            [3, 5, 1, 2, 4].map(|t| (api().slot_of)(h, t)),
            [0, 1, 2, 3, -1]
        );
        let tracks = [3, 5];
        assert_eq!((api().publish)(h, &plan(5), tracks.as_ptr(), 2), 3);
        let timeline = [1, 2];
        assert_eq!((api().publish)(h, &plan(1), timeline.as_ptr(), 2), 3);
        assert_eq!((api().publish)(h, &plan(3), tracks.as_ptr(), 2), 0);
    }
    for slot in [0, 2] {
        submit(h, slot, c"a", &file, CURRENT_GENERATION);
        assert_eq!(field(&wait_outcome(h), "error"), "\"\"");
    }

    let before = |track: i32, events: &mut [u8], position: &mut usize| unsafe {
        let capacity = events.len();
        (api().before)(h, track, 16, 100.0, events.as_mut_ptr(), position, capacity)
    };
    let mut kept = [0xabu8; 64];
    let mut kept_at = 5;
    assert!(
        before(1, &mut kept, &mut kept_at),
        "an engaged timeline slot sounds"
    );
    assert_eq!(
        (kept, kept_at),
        ([0xab; 64], 5),
        "the timeline events are untouched"
    );

    // Room for two of the three note-ons.
    let mut events = [0xabu8; 30];
    let mut position = 7;
    assert!(before(3, &mut events, &mut position));
    assert_eq!(position, 24);
    let words: Vec<u32> = events[..24]
        .chunks(4)
        .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(words, [note_on(60, 20000), note_on(64, 20000)].concat());
    assert_eq!(events[24..], [0xab; 6]);

    let mut silent = [0u8; 12];
    let mut silent_at = 0;
    assert!(
        !before(5, &mut silent, &mut silent_at),
        "an empty slot is silent"
    );
    assert!(!before(2, &mut silent, &mut silent_at));
    let mut samples = [1.0f32; 16];
    let channels = [samples.as_mut_ptr().cast::<c_void>()];
    unsafe {
        (api().after)(h, 3, channels.as_ptr(), 1, 16, false);
        for track in [1, 2, 5] {
            (api().after)(h, track, std::ptr::null(), 0, 16, false);
        }
        (api().destroy)(h);
    }
    assert!(samples.iter().all(|s| s.is_finite()));
}
