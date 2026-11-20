//! Several groups in one engine: each crossfades on its own, `Timeline`
//! groups leave the `Chord` groups alone, and a miss blames one slot across
//! all of them.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use repl_extension_api::sequence::Step;

use super::*;
use crate::{
    alloc_count::allocations,
    layout::{Input, Layout},
    loader::Loader,
    ownership::{FREE, READY},
    test_host::{kind, MockHost},
};

const RATE: f64 = 100.0;
const CHORD: u32 = 100;
const HOLD: u32 = 500;
const FADE: u32 = 100;

fn engine(groups: &[([i32; 2], Input)]) -> (Arc<Ownership>, Playback) {
    let layout = Layout::new(groups).unwrap();
    let ownership = Arc::new(Ownership::new(layout.slot_count()));
    let playback = Playback::new(layout, 5.0, 1.0, Arc::clone(&ownership)).unwrap();
    (ownership, playback)
}

/// A `Chord` group on 0 and 1, then a `Timeline` group on 2 and 3.
fn pad_and_scatter() -> (Arc<Ownership>, Playback) {
    engine(&[([0, 1], Input::Chord), ([2, 3], Input::Timeline)])
}

fn sequence(p: &Playback, tracks: [i32; 2]) {
    let mut plan = Plan {
        count: 1,
        track: tracks[0],
        seconds: 1.0,
        gate: 1.0,
        velocity: 20000,
        ..Default::default()
    };
    plan.steps[0] = Step {
        notes: [60, 64, 67, 71, 0, 0, 0, 0],
        count: 4,
    };
    assert_eq!(p.publish(&plan, &tracks), 0);
}

fn make_ready(ownership: &Ownership, slot: usize) {
    assert!(ownership.claim(slot));
    ownership.settle(slot, true);
}

/// What one block did to each slot.
#[derive(Debug, PartialEq)]
struct Heard {
    gains: Vec<Vec<f32>>,
    events: Vec<Vec<(u32, u8, bool)>>,
    output: Vec<Vec<f32>>,
}

/// One block with the same tone into every slot; `slots` limits what is
/// recorded.
fn block(p: &Playback, frames: u32, slots: usize) -> Heard {
    let n = frames as usize;
    unsafe { p.begin(frames, RATE) };
    let mut heard = Heard {
        gains: Vec::new(),
        events: Vec::new(),
        output: Vec::new(),
    };
    for slot in 0..p.layout().slot_count() {
        let mut events = Vec::new();
        let mut samples: Vec<f32> = (0..n).map(|i| 4.0 * ((i % 7) as f32 - 3.0)).collect();
        unsafe {
            p.render(slot, frames, RATE, |at, pitch, on, _| {
                events.push((at, pitch, on))
            });
            p.mix(slot, &mut [&mut samples[..]], n);
            p.finish(slot);
        }
        if slot < slots {
            heard.gains.push(p.crossfade(slot, n));
            heard.events.push(events);
            heard.output.push(samples);
        }
    }
    heard
}

fn run(p: &Playback, frames: u32) {
    for _ in 0..frames / 10 {
        block(p, 10, 0);
    }
}

fn states(ownership: &Ownership) -> Vec<u32> {
    (0..ownership.len()).map(|s| ownership.get(s)).collect()
}

#[test]
fn three_groups_each_go_from_submit_to_retire() {
    let host = Arc::new(MockHost::default());
    let (ownership, p) = engine(&[
        ([0, 1], Input::Chord),
        ([2, 3], Input::Timeline),
        ([4, 5], Input::Timeline),
    ]);
    let loader = Loader::spawn(host.clone(), Arc::clone(&ownership));
    let settle = |count: usize| {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut out = Vec::new();
        while out.len() < count {
            assert!(Instant::now() < deadline, "timed out");
            out.extend(loader.poll());
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(out.iter().all(|o| o.error.is_none() && !o.stale));
    };
    sequence(&p, [0, 1]);
    for slot in 0..6 {
        assert_ne!(loader.submit(slot, kind("a"), vec![slot], 0), 0);
    }
    settle(6);
    assert_eq!(states(&ownership), [READY; 6]);
    run(&p, HOLD + FADE + 20);
    assert_eq!(
        states(&ownership),
        [FREE, ENGAGED, FREE, ENGAGED, FREE, ENGAGED]
    );

    // Only the middle group gets a new patch, and only it switches back.
    loader.submit(2, kind("b"), vec![9], 0);
    settle(1);
    run(&p, HOLD + FADE + 20);
    assert_eq!(
        states(&ownership),
        [FREE, ENGAGED, ENGAGED, FREE, FREE, ENGAGED]
    );
    assert_eq!(loader.view().slots[2].as_ref().unwrap().0, kind("b"));
}

#[test]
fn a_timeline_group_leaves_the_chord_group_as_it_was() {
    let (pad_ownership, pad) = engine(&[([0, 1], Input::Chord)]);
    let (both_ownership, both) = pad_and_scatter();
    for (ownership, p) in [(&pad_ownership, &pad), (&both_ownership, &both)] {
        sequence(p, [0, 1]);
        make_ready(ownership, 0);
        make_ready(ownership, 1);
    }
    make_ready(&both_ownership, 2);
    let mut switches = 0;
    for step in 0..200 {
        // The timeline group keeps switching meanwhile.
        if step % 40 == 10 {
            for slot in [2, 3] {
                if both_ownership.get(slot) == FREE {
                    make_ready(&both_ownership, slot);
                    switches += 1;
                }
            }
        }
        assert_eq!(block(&pad, 10, 2), block(&both, 10, 2), "block {step}");
        for slot in 0..2 {
            let bits = |(l, p, t): (f32, f32, f32)| [l, p, t].map(f32::to_bits);
            assert_eq!(bits(pad.measured(slot)), bits(both.measured(slot)));
            assert_eq!(pad.gain(slot), both.gain(slot));
        }
    }
    assert!(switches >= 3, "the timeline group switched: {switches}");
    assert!(
        pad.measured(0).2.is_finite(),
        "the chord slots were measured"
    );
    assert!(
        both.measured(2).2.is_nan(),
        "a timeline slot is not measured"
    );
}

#[test]
fn a_timeline_slot_is_only_faded() {
    let (ownership, p) = pad_and_scatter();
    make_ready(&ownership, 2);
    // Unarmed chord groups do not keep the timeline group silent.
    let heard = block(&p, 10, 4);
    assert!(heard.events[2].is_empty());
    let input: Vec<f32> = (0..10).map(|i| 4.0 * ((i % 7) as f32 - 3.0)).collect();
    assert_eq!(heard.output[2], input, "no trim and no limiter at gain 1");
    assert!(heard.output[0].iter().all(|&s| s == 0.0));
    run(&p, HOLD);
    make_ready(&ownership, 3);
    let fade = block(&p, FADE / 2, 4);
    for frame in 0..(FADE / 2) as usize {
        assert_eq!(fade.gains[2][frame] + fade.gains[3][frame], 1.0);
        let tone = f64::from(4.0 * ((frame % 7) as f32 - 3.0));
        let expected = (tone * f64::from(fade.gains[3][frame])) as f32;
        assert_eq!(fade.output[3][frame], expected);
    }
    assert!(
        fade.gains[3].iter().any(|&g| g > 0.0),
        "the next one fades in at once"
    );
}

/// A block the host dropped.
fn dropped(p: &Playback, frames: u32) {
    unsafe { p.begin(frames, RATE) };
    for slot in 0..p.layout().slot_count() {
        unsafe { p.render(slot, frames, RATE, |_, _, _, _| {}) };
    }
}

#[test]
fn a_miss_blames_a_measuring_slot_in_any_group_first() {
    let (ownership, p) = pad_and_scatter();
    sequence(&p, [0, 1]);
    make_ready(&ownership, 2);
    make_ready(&ownership, 3);
    block(&p, 10, 0);
    make_ready(&ownership, 0);
    block(&p, 10, 0);
    dropped(&p, 10);
    block(&p, 10, 0);
    assert_eq!(states(&ownership), [FREE, FREE, ENGAGED, ENGAGED]);
}

#[test]
fn without_measuring_a_miss_blames_the_slot_fading_in() {
    let (ownership, p) = pad_and_scatter();
    sequence(&p, [0, 1]);
    make_ready(&ownership, 0);
    block(&p, CHORD, 0);
    make_ready(&ownership, 2);
    make_ready(&ownership, 3);
    run(&p, HOLD + FADE / 2);
    assert!(p.fading_out(2));
    dropped(&p, 10);
    block(&p, 10, 0);
    assert_eq!(states(&ownership), [ENGAGED, FREE, ENGAGED, FREE]);
    assert!(!p.fading_out(2));
    assert!(doomed(&p).iter().all(Option::is_none), "nothing was marked");
}

fn doomed(p: &Playback) -> Vec<Option<usize>> {
    let m = unsafe { &*p.mix.get() };
    m.groups.iter().map(|g| g.doomed).collect()
}

#[test]
fn a_miss_marks_the_lone_current_slot_of_every_group() {
    let (ownership, p) = pad_and_scatter();
    sequence(&p, [0, 1]);
    make_ready(&ownership, 0);
    make_ready(&ownership, 2);
    block(&p, CHORD, 0);
    dropped(&p, 10);
    block(&p, 10, 0);
    assert_eq!(states(&ownership), [ENGAGED, FREE, ENGAGED, FREE]);
    assert_eq!(doomed(&p), [Some(0), Some(2)]);
    // Marked groups fade out as soon as their next slot may fade in, well
    // before the hold.
    make_ready(&ownership, 3);
    make_ready(&ownership, 1);
    run(&p, 3 * CHORD);
    assert_eq!(states(&ownership), [FREE, ENGAGED, FREE, ENGAGED]);
}

#[test]
fn several_groups_do_not_allocate_in_the_audio_callbacks() {
    let (ownership, p) = pad_and_scatter();
    sequence(&p, [0, 1]);
    for slot in 0..4 {
        make_ready(&ownership, slot);
    }
    let mut samples = [1.0f32; 64];
    let mut run = |frames: u32| {
        unsafe { p.begin(frames, RATE) };
        for slot in 0..4 {
            unsafe {
                p.render(slot, frames, RATE, |_, _, _, _| {});
                p.mix(
                    slot,
                    &mut [&mut samples[..frames as usize]],
                    frames as usize,
                );
                p.finish(slot);
            }
        }
    };
    let before = allocations();
    for _ in 0..20 {
        run(64);
    }
    assert_eq!(allocations(), before);
    assert_eq!(states(&ownership), [FREE, ENGAGED, FREE, ENGAGED]);
}
