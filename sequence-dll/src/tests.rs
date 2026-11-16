use super::{clock::Clock, voicing};
use repl_extension_api::sequence::{Plan, Step};

fn plan(chords: &[&[u8]], seconds: f64, gate: f64) -> Plan {
    let mut plan = Plan {
        count: chords.len() as u32,
        seconds,
        gate,
        velocity: 24000,
        ..Plan::default()
    };
    for (step, notes) in plan.steps.iter_mut().zip(chords) {
        step.count = notes.len() as u8;
        step.notes[..notes.len()].copy_from_slice(notes);
    }
    plan
}

#[test]
fn fractional_sample_clock_has_no_block_drift() {
    let clock = Clock::default();
    let seconds = 60.0 / 137.0 / 4.0;
    assert_eq!(
        clock.publish(&plan(&[&[60], &[64], &[67]], seconds, 0.7)),
        0
    );
    let mut ons = Vec::new();
    for block in 0..10_000 {
        unsafe {
            clock.render(0, 511, 48000.0, |at, pitch, on, _| {
                if on {
                    ons.push((block * 511 + at, pitch));
                }
            });
        }
    }
    for (i, (frame, pitch)) in ons.iter().enumerate() {
        assert!((f64::from(*frame) - i as f64 * seconds * 48000.0).abs() <= 1.0);
        assert_eq!(*pitch, [60, 64, 67][i % 3]);
    }
    assert!(ons.len() > 900);
}

#[test]
fn full_gate_ties_common_notes_and_changes_other_notes_at_the_same_sample() {
    let clock = Clock::default();
    assert_eq!(clock.publish(&plan(&[&[60, 64], &[60, 67]], 0.5, 1.0)), 0);
    let mut events = Vec::new();
    unsafe {
        clock.render(0, 1100, 1000.0, |at, pitch, on, _| {
            events.push((at, pitch, on))
        });
    }
    assert_eq!(
        events,
        [
            (0, 60, true),
            (0, 64, true),
            (500, 64, false),
            (500, 67, true),
            (1000, 67, false),
            (1000, 64, true)
        ]
    );
}

#[test]
fn live_edits_keep_phase_and_wait_for_a_boundary() {
    let clock = Clock::default();
    assert_eq!(clock.publish(&plan(&[&[60], &[64]], 0.5, 1.0)), 0);
    unsafe {
        clock.render(0, 123, 1000.0, |_, _, _, _| {});
    }
    assert_eq!(clock.publish(&plan(&[&[67], &[69]], 0.5, 1.0)), 0);
    let mut events = Vec::new();
    unsafe {
        clock.render(0, 1000, 1000.0, |at, pitch, on, _| {
            events.push((at, pitch, on))
        });
    }
    assert_eq!(
        events,
        [
            (377, 60, false),
            (377, 69, true),
            (877, 69, false),
            (877, 67, true)
        ]
    );
    unsafe {
        clock.rewind();
    }
    let mut first = Vec::new();
    unsafe {
        clock.render(0, 1, 1000.0, |_, pitch, on, _| {
            if on {
                first.push(pitch);
            }
        });
    }
    assert_eq!(first, [67]);
}

#[test]
fn bypassed_tracks_keep_phase_and_prime_all_voices_after_loading() {
    let clock = Clock::default();
    assert_eq!(clock.publish(&plan(&[&[60, 64], &[60, 67]], 0.5, 1.0)), 0);
    unsafe {
        clock.render_enabled(0, 750, 1000.0, false, |_, _, _, _| {
            panic!("Bypassed track emitted notes")
        });
    }
    let mut events = Vec::new();
    unsafe {
        clock.render_enabled(0, 300, 1000.0, true, |at, pitch, on, _| {
            events.push((at, pitch, on))
        });
    }
    assert_eq!(events, [(250, 60, true), (250, 64, true)]);
}

#[test]
fn resetting_a_synth_reprimes_common_notes_without_rewinding_the_clock() {
    let clock = Clock::default();
    assert_eq!(clock.publish(&plan(&[&[60, 64], &[60, 67]], 0.5, 1.0)), 0);
    unsafe {
        clock.render(0, 250, 1000.0, |_, _, _, _| {});
        clock.release();
    }
    let mut events = Vec::new();
    unsafe {
        clock.render(0, 300, 1000.0, |at, pitch, on, _| {
            events.push((at, pitch, on))
        });
    }
    assert_eq!(events, [(250, 60, true), (250, 67, true)]);
}

#[test]
fn invalid_updates_and_queue_backpressure_preserve_the_plan() {
    let clock = Clock::default();
    let valid = plan(&[&[60]], 0.5, 1.0);
    for _ in 0..4 {
        assert_eq!(clock.publish(&valid), 0);
    }
    assert_eq!(clock.publish(&valid), 2);
    let mut invalid = valid;
    invalid.steps[0] = Step {
        notes: [128; 8],
        count: 1,
    };
    assert_eq!(clock.publish(&invalid), 1);
    unsafe {
        clock.reset();
    }
    assert_eq!(clock.publish(&valid), 0);
}

#[test]
fn solver_checks_seam_mixed_edges_and_pinned_sounding_chord() {
    let old = vec![vec![60, 64], vec![62, 65], vec![64, 67]];
    let candidates = vec![
        vec![vec![72, 76], vec![60, 64]],
        vec![vec![65, 69], vec![62, 65]],
        vec![vec![67, 71], vec![64, 67]],
    ];
    let path = voicing::choose(&candidates, &old, 7, Some(1)).unwrap();
    let next: Vec<_> = path
        .iter()
        .enumerate()
        .map(|(i, j)| candidates[i][*j].clone())
        .collect();
    assert_eq!(next[1], old[1]);
    for i in 0..3 {
        for (a, b) in [
            (&next[i], &next[(i + 1) % 3]),
            (&next[i], &old[(i + 1) % 3]),
            (&old[i], &next[(i + 1) % 3]),
        ] {
            assert!(a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 7));
        }
    }
    assert!(voicing::choose(
        &[vec![vec![60]], vec![vec![67]], vec![vec![74]]],
        &[],
        7,
        None
    )
    .is_none());
}

#[test]
fn callback_cost_is_small_relative_to_the_audio_it_renders() {
    let clock = Clock::default();
    assert_eq!(
        clock.publish(&plan(&[&[48, 52, 55, 59], &[50, 53, 57, 60]], 0.05, 1.0)),
        0
    );
    let start = std::time::Instant::now();
    let mut events = 0;
    for _ in 0..10_000 {
        unsafe {
            clock.render(0, 512, 48000.0, |_, _, _, _| events += 1);
        }
    }
    let elapsed = start.elapsed();
    eprintln!("Rust sequence: 106.7s audio / 10000 callbacks in {elapsed:?}, {events} events");
    assert!(elapsed.as_secs_f64() < 1.0);
}
