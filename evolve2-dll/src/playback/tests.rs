use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use repl_extension_api::sequence::Step;

use super::*;
use crate::{
    alloc_count::allocations,
    loader::Loader,
    ownership::{FREE, LOADING, READY},
    test_host::{kind, Call, MockHost},
};

const RATE: f64 = 100.0;
/// One chord at `RATE`: a ready slot is primed after this many frames.
const CHORD: u32 = 100;
const HOLD: u32 = 500;
const FADE: u32 = 100;

fn engine() -> (Arc<Ownership>, Playback) {
    let ownership = Arc::new(Ownership::default());
    let playback = Playback::new(5.0, 1.0, Arc::clone(&ownership)).unwrap();
    (ownership, playback)
}

fn sequence(p: &Playback) {
    let mut plan = Plan {
        count: 1,
        seconds: 1.0,
        gate: 1.0,
        velocity: 20000,
        ..Default::default()
    };
    plan.steps[0] = Step {
        notes: [60, 64, 67, 71, 0, 0, 0, 0],
        count: 4,
    };
    assert_eq!(p.publish(&plan), 0);
}

/// Loader-side hand-over without a loader.
fn make_ready(ownership: &Ownership, slot: usize) {
    assert!(ownership.claim(slot));
    ownership.settle(slot, true);
}

fn block(p: &Playback, frames: u32) -> [Vec<f32>; 2] {
    unsafe { p.begin(frames, RATE) };
    let mut gains = [vec![1.0; frames as usize], vec![1.0; frames as usize]];
    for (slot, gain) in gains.iter_mut().enumerate() {
        unsafe {
            p.render(slot, frames, RATE, |_, _, _, _| {});
            p.mix(slot, gain);
            p.finish(slot);
        }
    }
    gains
}

fn constant(gains: &[f32], value: f32) -> bool {
    gains.iter().all(|&g| g == value)
}

#[test]
fn only_a_ready_slot_enters_the_fade() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);

    // The next slot is free, then loading: the current one keeps sounding.
    let free = block(&p, HOLD + 2 * CHORD);
    assert!(constant(&free[0], 1.0) && constant(&free[1], 0.0));
    assert!(ownership.claim(1));
    let loading = block(&p, 2 * HOLD);
    assert!(constant(&loading[0], 1.0) && constant(&loading[1], 0.0));
    assert_eq!(ownership.get(1), LOADING);

    ownership.settle(1, true);
    assert_eq!(ownership.get(1), READY);
    let primed = block(&p, CHORD);
    assert!(constant(&primed[0], 1.0), "a whole chord primes first");
    let fade = block(&p, FADE + 1);
    for (&a, &b) in fade[0].iter().zip(&fade[1]) {
        assert!((a + b - 1.0).abs() < 1e-6);
    }
    assert_eq!(*fade[0].last().unwrap(), 0.0);
    assert_eq!(*fade[1].last().unwrap(), 1.0);
    assert_eq!(ownership.get(0), FREE, "the faded-out slot goes back");
    assert_eq!(ownership.get(1), ENGAGED);

    // A slot that sounded before and is loading again is not ready either.
    assert!(ownership.claim(0));
    let reloading = block(&p, 2 * HOLD);
    assert!(constant(&reloading[0], 0.0) && constant(&reloading[1], 1.0));
}

#[test]
fn a_ready_next_never_shortens_the_hold() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    make_ready(&ownership, 1);
    let hold = block(&p, HOLD);
    assert!(constant(&hold[0], 1.0) && constant(&hold[1], 0.0));
    let fade = block(&p, 1);
    assert!(fade[1][0] == 0.0 && p.fading_out(0));
}

#[test]
fn a_retiring_slot_releases_its_notes_before_it_goes_back() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    make_ready(&ownership, 1);
    block(&p, HOLD);
    unsafe { p.begin(FADE + 1, RATE) };
    let mut offs = 0;
    unsafe { p.render(0, FADE + 1, RATE, |_, _, on, _| offs += usize::from(!on)) };
    assert_eq!(offs, 4);
    assert_eq!(ownership.get(0), ENGAGED, "still owned until finish");
    unsafe { p.finish(0) };
    assert_eq!(ownership.get(0), FREE);
    unsafe {
        p.render(1, FADE + 1, RATE, |_, _, _, _| {});
        p.finish(1);
        p.begin(10, RATE);
        p.render(0, 10, RATE, |_, _, _, _| panic!("a silent slot emitted"));
    }
}

#[test]
fn a_submit_during_the_fade_does_not_change_the_sounding_slots() {
    let host = Arc::new(MockHost::default());
    let (ownership, p) = engine();
    let loader = Loader::spawn(host.clone(), Arc::clone(&ownership));
    let settle = |count: usize| {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut out = Vec::new();
        while out.len() < count {
            assert!(Instant::now() < deadline, "timed out");
            out.extend(loader.poll());
            std::thread::sleep(Duration::from_millis(1));
        }
        out
    };
    sequence(&p);
    loader.submit(0, kind("a"), vec![1], 0);
    loader.submit(1, kind("b"), vec![2], 0);
    assert!(settle(2).iter().all(|o| o.error.is_none() && !o.stale));
    block(&p, HOLD);
    block(&p, FADE / 2);
    assert!(p.fading_out(0));
    let sounding = (host.attached(0), host.attached(1));
    let loads = host.count(|c| matches!(c, Call::Load(_)));

    loader.submit(0, kind("c"), vec![3], 0);
    std::thread::sleep(Duration::from_millis(100));

    assert_eq!((host.attached(0), host.attached(1)), sounding);
    assert_eq!(host.count(|c| matches!(c, Call::Load(_))), loads);
    assert!(loader.poll().is_empty());
    assert_eq!((ownership.get(0), ownership.get(1)), (ENGAGED, ENGAGED));

    // The fade ends, slot 0 goes back, and only then is the request applied.
    block(&p, FADE);
    let outcome = settle(1).remove(0);
    assert!(outcome.error.is_none());
    assert_eq!(host.attached(1), sounding.1);
    assert_ne!(host.attached(0), sounding.0);
    assert_eq!(loader.view().slots[0].as_ref().unwrap().0, kind("c"));
}

#[test]
fn the_audio_callbacks_do_not_allocate() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    let mut samples = [1.0f32; 64];
    let mut wide = [1.0f64; 64];
    let mut events = 0usize;
    let mut run = |frames: u32| {
        unsafe { p.begin(frames, RATE) };
        for slot in 0..SLOT_COUNT {
            unsafe {
                p.render(slot, frames, RATE, |_, _, _, _| events += 1);
                p.mix(slot, &mut samples[..frames as usize]);
                p.mix64(slot, &mut wide[..frames as usize]);
                p.finish(slot);
            }
        }
    };
    let before = allocations();
    for _ in 0..10 {
        run(64);
    }
    make_ready(&ownership, 1);
    // Through engage, prime, the whole fade and the retirement.
    for _ in 0..20 {
        run(64);
    }
    assert_eq!(allocations(), before);
    assert_eq!(ownership.get(0), FREE, "the run covered a retirement");
    assert!(events > 0);
}
