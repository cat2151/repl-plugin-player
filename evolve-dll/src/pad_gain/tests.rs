use super::*;
use crate::level::CEILING;

const RATE: f64 = 48_000.0;
const FRAMES: usize = 480;
const ON: [u32; 2] = [0x4090_3c00, 0x8000_0000];
const OFF: [u32; 2] = [0x4080_3c00, 0];
const MIDI1_ON: u32 = 0x2091_3d40;
const MIDI1_OFF: u32 = 0x2091_3d00;
const CHORD_ON: [u32; 3] = [ON[0], ON[1], MIDI1_ON];
const CHORD_OFF: [u32; 3] = [OFF[0], OFF[1], MIDI1_OFF];

fn render(id: i32, raw: f64, events: &[u32]) -> (f64, f64) {
    let mut left = [raw; FRAMES];
    let mut right = [-raw * 0.5; FRAMES];
    let out = [left.as_mut_ptr().cast(), right.as_mut_ptr().cast()];
    unsafe {
        repl_pad_gain_process(
            id,
            out.as_ptr(),
            out.len(),
            FRAMES,
            RATE,
            true,
            events.as_ptr(),
            events.len(),
        )
    };
    (left[FRAMES - 1], right[FRAMES - 1])
}

fn hold(id: i32, raw: f64, events: &[u32]) -> (f64, f64) {
    render(id, raw, events);
    for _ in 0..30 {
        render(id, raw, &[]);
    }
    render(id, raw, &[])
}

#[test]
fn boosts_the_first_chord_and_readapts_to_a_single_note_per_instance() {
    const QUIET: i32 = 3;
    const LOUD: i32 = 7;
    const OTHER: i32 = 10;
    let configure = |ids: &[i32]| assert!(unsafe { repl_pad_gain_set(ids.as_ptr(), ids.len()) });
    configure(&[QUIET, LOUD]);
    assert!(repl_pad_gain_read(QUIET).is_nan());
    let chord = hold(QUIET, 0.12, &CHORD_ON).0;
    assert!((chord - 0.251).abs() < 0.003, "{chord}");
    let chord_gain = repl_pad_gain_read(QUIET);
    let released = render(QUIET, 0.07, &CHORD_OFF).0;
    assert!((released - 0.07 * f64::from(chord_gain)).abs() < 1e-7);
    assert_eq!(repl_pad_gain_read(QUIET), chord_gain);
    assert_eq!(hold(LOUD, 0.40, &ON).0, 0.40);

    let (boosted, right) = hold(QUIET, 0.05, &ON);
    assert!((boosted - 0.251).abs() < 0.003, "{boosted}");
    assert!((f64::from(repl_pad_gain_read(QUIET)) - boosted / 0.05).abs() < 1e-6);
    assert_eq!(repl_pad_gain_read(LOUD), 1.0);
    assert_eq!(right, -boosted * 0.5);
    assert!(repl_pad_gain_read(QUIET) > 2.0 * chord_gain);
    assert_eq!(render(QUIET, 0.60, &[MIDI1_ON]).0, 0.60);
    assert_eq!(repl_pad_gain_read(QUIET), 1.0);
    assert_eq!(render(LOUD, 0.40, &[]).0, 0.40);
    assert_eq!(render(OTHER, 0.01, &ON).0, 0.01);

    let (left, right) = render(MASTER, 1.8, &[]);
    assert!((left - CEILING).abs() < 1e-12);
    assert_eq!(right, -left * 0.5);
    assert!((f64::from(repl_pad_gain_read(MASTER)) - left / 1.8).abs() < 1e-7);
    render(MASTER, 0.0, &[]);
    assert!(repl_pad_gain_read(MASTER) > (left / 1.8) as f32);

    configure(&[]);
    assert!(repl_pad_gain_read(QUIET).is_nan());
    assert!(repl_pad_gain_read(MASTER).is_nan());
    assert_eq!(render(MASTER, 1.8, &[]).0, 1.8);
    assert_eq!(render(QUIET, 0.05, &[]).0, 0.05);
    configure(&[QUIET]);
    assert!(repl_pad_gain_read(QUIET).is_nan());
    assert_eq!(render(QUIET, 0.05, &[]).0, 0.05);
    assert_eq!(repl_pad_gain_read(QUIET), 1.0);
    configure(&[]);
}

#[test]
fn release_stops_an_unfinished_gain_increase_and_can_reduce_it() {
    let mut measurement = Measurement::new(RATE);
    measurement.events(&ON);
    assert!(measurement.observe(0.025, FRAMES * 4, 1.0) > 2.0);
    measurement.events(&OFF);
    assert_eq!(measurement.observe(0.02, FRAMES, 2.0), 2.0);
    assert_eq!(measurement.observe(0.001, FRAMES * 100, 2.0), 2.0);
    assert_eq!(measurement.observe(0.4, FRAMES, 2.0), 1.0);
    assert_eq!(measurement.observe(0.001, FRAMES * 100, 1.0), 1.0);
}

#[test]
fn held_chord_keeps_its_peak_until_the_next_note_on() {
    let mut measurement = Measurement::new(RATE);
    measurement.events(&CHORD_ON);
    assert_eq!(measurement.observe(0.10, FRAMES * 3, 1.0), 1.0);
    let chord_gain = measurement.observe(0.12, FRAMES, 1.0);
    assert!((chord_gain * 0.12 - 0.251188643150958).abs() < 1e-12);
    assert_eq!(
        measurement.observe(0.015, FRAMES * 100, chord_gain),
        chord_gain
    );
    measurement.events(&OFF); // One chord note is still held.
    assert_eq!(
        measurement.observe(0.012, FRAMES * 100, chord_gain),
        chord_gain
    );

    measurement.events(&[MIDI1_ON]);
    assert_eq!(
        measurement.observe(0.04, FRAMES * 3, chord_gain),
        chord_gain
    );
    let single_gain = measurement.observe(0.04, FRAMES, chord_gain);
    assert!(single_gain > 2.0 * chord_gain);
    measurement.events(&[MIDI1_OFF]);
    assert_eq!(
        measurement.observe(0.01, FRAMES * 100, single_gain),
        single_gain
    );
}

#[test]
fn noise_and_silence_do_not_raise_gain_after_a_note_on() {
    let mut measurement = Measurement::new(RATE);
    assert_eq!(measurement.observe(0.05, FRAMES * 12, 1.0), 1.0);
    assert_eq!(measurement.observe(0.0005, FRAMES, 1.0), 1.0);
    measurement.events(&ON);
    assert_eq!(measurement.observe(0.0005, FRAMES * 12, 1.0), 1.0);
    assert_eq!(measurement.observe(0.00001, FRAMES * 12, 1.0), 1.0);
    assert!((20.0 * measurement.observe(0.006, FRAMES, 1.0).log10() - 18.0).abs() < 1e-10);
}

#[test]
fn a_single_note_recovers_under_a_chord_tail_but_a_repeat_does_not_follow_its_decay() {
    let settle = |first: &[u32], off: &[u32]| {
        let mut measurement = Measurement::new(RATE);
        measurement.events(first);
        let mut gain = measurement.observe(0.12, FRAMES * 10, 1.0);
        measurement.events(off);
        measurement.events(&ON);
        for _ in 0..30 {
            gain = measurement.observe(0.10, FRAMES, gain);
        }
        for _ in 0..30 {
            gain = measurement.observe(0.05, FRAMES, gain);
        }
        // The re-measurement ends; a later fade is not followed.
        measurement.observe(0.05, FRAMES * 200, gain);
        for _ in 0..100 {
            gain = measurement.observe(0.02, FRAMES, gain);
        }
        gain
    };
    let after_chord = settle(&CHORD_ON, &CHORD_OFF);
    assert!((after_chord * 0.05 - 0.251).abs() < 0.001, "{after_chord}");
    let repeat = settle(&ON, &OFF);
    assert!((repeat * 0.10 - 0.251).abs() < 0.001, "{repeat}");
}
