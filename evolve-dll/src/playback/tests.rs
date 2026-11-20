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
/// One chord at `RATE`: a slot is measured this many frames after its first note-on.
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

/// The crossfade gains of one block; the slots output silence.
fn block(p: &Playback, frames: u32) -> [Vec<f32>; 2] {
    unsafe { p.begin(frames, RATE) };
    let gains = [
        p.crossfade(0, frames as usize),
        p.crossfade(1, frames as usize),
    ];
    for slot in 0..SLOT_COUNT {
        let mut silence = vec![0.0f32; frames as usize];
        unsafe {
            p.render(slot, frames, RATE, |_, _, _, _| {});
            p.mix(slot, &mut [&mut silence[..]], frames as usize);
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
    let measuring = block(&p, CHORD);
    assert!(
        constant(&measuring[0], 1.0),
        "the first slot sounds while measured"
    );

    // The next slot is free, then loading: the current one keeps sounding.
    let free = block(&p, HOLD + 2 * CHORD);
    assert!(constant(&free[0], 1.0) && constant(&free[1], 0.0));
    assert!(ownership.claim(1));
    let loading = block(&p, 2 * HOLD);
    assert!(constant(&loading[0], 1.0) && constant(&loading[1], 0.0));
    assert_eq!(ownership.get(1), LOADING);

    ownership.settle(1, true);
    assert_eq!(ownership.get(1), READY);
    let measuring = block(&p, CHORD);
    assert!(
        constant(&measuring[0], 1.0),
        "a whole chord is measured first"
    );
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
fn only_the_current_slot_starts_the_chord_in_progress() {
    let (ownership, p) = engine();
    sequence(&p);
    block(&p, CHORD / 2);
    make_ready(&ownership, 0);
    make_ready(&ownership, 1);
    unsafe { p.begin(1, RATE) };
    let mut ons = [Vec::new(), Vec::new()];
    for (slot, events) in ons.iter_mut().enumerate() {
        unsafe {
            p.render(slot, 1, RATE, |at, pitch, on, _| {
                events.push((at, pitch, on))
            })
        };
    }
    assert_eq!(
        ons[0],
        [(0, 60, true), (0, 64, true), (0, 67, true), (0, 71, true)]
    );
    assert!(ons[1].is_empty(), "the next slot waits for the next chord");
}

#[test]
fn a_ready_next_never_shortens_the_hold() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    make_ready(&ownership, 1);
    // The hold counts from the first slot's first chord.
    block(&p, CHORD);
    let hold = block(&p, HOLD - CHORD);
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
    block(&p, CHORD);
    block(&p, HOLD - CHORD);
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
    block(&p, CHORD);
    block(&p, HOLD - CHORD);
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
                let n = frames as usize;
                p.mix(slot, &mut [&mut samples[..n]], n);
                p.mix(slot, &mut [&mut wide[..n]], n);
                p.finish(slot);
            }
        }
    };
    let before = allocations();
    for _ in 0..10 {
        run(64);
    }
    make_ready(&ownership, 1);
    // Through engage, measuring, the whole fade and the retirement.
    for _ in 0..20 {
        run(64);
    }
    assert_eq!(allocations(), before);
    assert_eq!(ownership.get(0), FREE, "the run covered a retirement");
    assert!(events > 0);
}

#[test]
fn the_heard_output_is_trimmed_and_within_the_ceiling() {
    use crate::level::{CEILING, MAX_TRIM_DB, TARGET_LUFS};
    const RATE: f64 = 48_000.0;
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    let tone = |frame: usize, amplitude: f32| {
        amplitude * (std::f32::consts::TAU * 997.0 * frame as f32 / RATE as f32).sin()
    };
    let mut frame = 0;
    let mut peak = 0.0f32;
    // Quiet for the measured chord, then swelling far past full scale.
    for amplitude in [0.01f32; 50].into_iter().chain([4.0; 50]) {
        unsafe { p.begin(1024, RATE) };
        let mut left: Vec<f32> = (frame..frame + 1024).map(|i| tone(i, amplitude)).collect();
        let mut right = left.clone();
        unsafe {
            p.render(0, 1024, RATE, |_, _, _, _| {});
            p.mix(0, &mut [&mut left[..], &mut right[..]], 1024);
            p.finish(0);
        }
        if p.measured(0).0.is_finite() {
            peak = left.iter().chain(&right).fold(peak, |m, x| m.max(x.abs()));
        }
        frame += 1024;
    }
    let (loudness, _, trim) = p.measured(0);
    // -40 dBFS in both channels, raised as far as the trim goes.
    assert!((f64::from(loudness) - -40.0).abs() < 0.1, "{loudness}");
    let expected = 10f64.powf((TARGET_LUFS - f64::from(loudness)).min(MAX_TRIM_DB) / 20.0);
    assert!((f64::from(trim) - expected).abs() < 1e-3, "{trim}");
    assert!(peak > 0.0 && f64::from(peak) <= CEILING + 1e-6, "{peak}");
}

/// A block the host dropped: `begin` and `render`, but no `mix` or `finish`.
fn dropped(p: &Playback, frames: u32) {
    unsafe { p.begin(frames, RATE) };
    for slot in 0..SLOT_COUNT {
        unsafe { p.render(slot, frames, RATE, |_, _, _, _| {}) };
    }
}

#[test]
fn a_dropped_block_retires_the_measuring_slot() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    block(&p, 10);
    dropped(&p, 10);
    block(&p, 10);
    assert_eq!(ownership.get(0), FREE);
}

#[test]
fn a_miss_after_the_next_slot_engages_retires_only_that_slot() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    block(&p, CHORD);
    block(&p, HOLD);
    make_ready(&ownership, 1);
    block(&p, 10);
    dropped(&p, 10);
    let after = block(&p, 10);
    assert_eq!((ownership.get(0), ownership.get(1)), (ENGAGED, FREE));
    assert!(constant(&after[0], 1.0) && constant(&after[1], 0.0));
}

#[test]
fn a_late_block_retires_the_slot() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    // At `RATE`, a block of one frame lasts 10ms.
    unsafe { p.begin(1, RATE) };
    std::thread::sleep(Duration::from_millis(20));
    for slot in 0..SLOT_COUNT {
        unsafe {
            p.render(slot, 1, RATE, |_, _, _, _| {});
            p.finish(slot);
        }
    }
    block(&p, 10);
    assert_eq!(ownership.get(0), FREE);
}

#[test]
fn a_miss_while_sounding_alone_fades_out_once_the_next_is_measured() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    block(&p, CHORD);
    dropped(&p, 10);
    let kept = block(&p, 10);
    assert_eq!(ownership.get(0), ENGAGED);
    assert!(constant(&kept[0], 1.0), "no silence before a next slot");
    make_ready(&ownership, 1);
    block(&p, CHORD);
    let early = block(&p, 1);
    assert!(
        p.fading_out(0) && early[1][0] == 0.0,
        "the hold is cut short"
    );
}

#[test]
fn misses_while_marked_retire_nothing() {
    let (ownership, p) = engine();
    sequence(&p);
    make_ready(&ownership, 0);
    block(&p, CHORD);
    dropped(&p, 10);
    block(&p, 10);
    make_ready(&ownership, 1);
    block(&p, 10);
    dropped(&p, 10);
    let after = block(&p, 10);
    assert_eq!((ownership.get(0), ownership.get(1)), (ENGAGED, ENGAGED));
    assert!(constant(&after[0], 1.0));
}
