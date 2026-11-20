use super::*;

const RATE: f64 = 48_000.0;

fn sine(amplitude: f64, hz: f64, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| (amplitude * (std::f64::consts::TAU * hz * i as f64 / RATE).sin()) as f32)
        .collect()
}

/// Measures `left`/`right` for their whole length, then fixes the trim.
fn measured(left: &[f32], right: &[f32]) -> Level {
    let mut level = Level::default();
    level.prepare(RATE);
    level.note_on();
    let (mut l, mut r) = (left.to_vec(), right.to_vec());
    let frames = l.len();
    level.process(&mut [&mut l[..], &mut r[..]], frames, &vec![0.0; frames]);
    assert!(level.settle(frames as u64, false));
    level
}

#[test]
fn the_48_khz_filters_are_the_bs_1770_table() {
    let (s, h) = (Biquad::shelf(RATE), Biquad::high_pass(RATE));
    let table = [
        (s.b0, 1.535_124_859_586_97),
        (s.b1, -2.691_696_189_406_38),
        (s.b2, 1.198_392_810_852_85),
        (s.a1, -1.690_659_293_182_41),
        (s.a2, 0.732_480_774_215_85),
        (h.a1, -1.990_047_454_833_98),
        (h.a2, 0.990_072_250_366_21),
    ];
    for (got, expected) in table {
        assert!((got - expected).abs() < 1e-9, "{got} {expected}");
    }
}

#[test]
fn a_full_scale_997_hz_sine_reads_like_bs_1770() {
    // BS.1770-4: a 0 dBFS 997 Hz sine in one channel reads -3.01 LKFS.
    let level = measured(&sine(1.0, 997.0, 48_000), &[0.0; 48_000]);
    assert!(
        (level.loudness() - -3.01).abs() < 0.05,
        "{}",
        level.loudness()
    );
    assert!(level.peak_db().abs() < 0.01);
}

#[test]
fn the_trim_brings_the_loudness_to_the_target_within_its_range() {
    for (amplitude, expected) in [(0.1, None), (0.3, None), (1e-3, Some(MAX_TRIM_DB))] {
        let tone = sine(amplitude, 997.0, 48_000);
        let level = measured(&tone, &tone);
        let db = 20.0 * level.trim.unwrap().log10();
        match expected {
            Some(limit) => assert!((db - limit).abs() < 1e-9, "{db}"),
            None => assert!((db - (TARGET_LUFS - level.loudness())).abs() < 1e-9, "{db}"),
        }
    }
}

#[test]
fn a_silent_slot_is_not_trimmed() {
    let level = measured(&[0.0; 4800], &[0.0; 4800]);
    assert_eq!(level.trim, Some(1.0));
}

#[test]
fn measuring_waits_for_the_first_note_on() {
    let mut level = Level::default();
    level.prepare(RATE);
    let mut loud = sine(1.0, 997.0, 4800);
    level.process(&mut [&mut loud[..]], 4800, &[0.0; 4800]);
    assert!(!level.settle(1, false), "nothing sounded yet");
    level.note_on();
    let mut quiet = sine(0.01, 997.0, 4800);
    level.process(&mut [&mut quiet[..]], 4800, &[0.0; 4800]);
    assert!(level.settle(4800, false));
    assert!(level.loudness() < -40.0, "{}", level.loudness());
}

#[test]
fn the_limiter_keeps_every_sample_within_the_ceiling() {
    let tone = sine(0.05, 120.0, 48_000);
    let mut level = measured(&tone, &tone);
    // Much louder than measured, as a patch swelling after its first chord.
    let mut left = sine(4.0, 120.0, 48_000);
    let mut right = sine(2.0, 120.0, 48_000);
    level.process(&mut [&mut left[..], &mut right[..]], 48_000, &[1.0; 48_000]);
    let peak = left
        .iter()
        .chain(&right)
        .fold(0.0f32, |p, x| p.max(x.abs()));
    assert!(f64::from(peak) <= CEILING + 1e-6, "{peak}");
}

#[test]
fn the_limiter_recovers_after_a_peak() {
    let tone = sine(0.1, 997.0, 4800);
    let mut level = measured(&tone, &tone);
    let mut burst = [1000.0f32; 1];
    level.process(&mut [&mut burst[..]], 1, &[1.0]);
    let mut quiet = vec![0.0f32; (RATE * RELEASE_SECONDS * 10.0) as usize];
    let frames = quiet.len();
    level.process(&mut [&mut quiet[..]], frames, &vec![1.0; frames]);
    assert!(level.envelope > 0.999, "{}", level.envelope);
}

#[test]
fn a_slot_heard_while_measured_glides_to_its_trim() {
    let quiet = sine(0.01, 997.0, 4800);
    for heard in [true, false] {
        let mut level = Level::default();
        level.prepare(RATE);
        level.note_on();
        let mut measuring = quiet.clone();
        level.process(&mut [&mut measuring[..]], 4800, &[1.0; 4800]);
        assert_eq!(measuring, quiet, "untrimmed while measured");
        assert!(level.settle(4800, heard));
        let glide = (GLIDE_SECONDS * RATE) as usize;
        let mut ones = vec![0.001f32; glide + 1];
        level.process(&mut [&mut ones[..]], glide + 1, &vec![1.0; glide + 1]);
        let scale = |x: f32| f64::from(x) / 0.001;
        let trim = level.trim;
        if heard {
            assert!((scale(ones[0]) - 1.0).abs() < 1e-3, "{}", scale(ones[0]));
            assert!(ones.windows(2).all(|w| w[1] >= w[0]), "no step");
        } else {
            assert!((scale(ones[0]) - trim.unwrap()).abs() < 1e-3);
        }
        assert!((scale(ones[glide]) - trim.unwrap()).abs() < 1e-3);
    }
}
