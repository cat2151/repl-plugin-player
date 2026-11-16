use super::*;
use crate::scope::Display;

#[test]
fn master_mix_uses_fixed_windows_and_resets_on_source_change() {
    let mut stream = Stream::new(8_000);
    feed(&mut stream, 1, 69, 0, 10);
    let frames = feed(&mut stream, 2, 129, 10, 320);
    assert_eq!(frames.len(), 2);
    for frame in frames {
        assert_eq!(frame.samples.len(), 160);
        assert_eq!(frame.shift_samples, 0.0);
        assert!(frame.samples.iter().any(|s| s[0] != 0.0));
    }
    let width = crate::scope_trigger::width(8_000, 69);
    assert_eq!(feed(&mut stream, 3, 69, 330, width).len(), 1);
}

fn feed(stream: &mut Stream, epoch: u64, note: u8, serial: u64, count: usize) -> Vec<Display> {
    (0..count)
        .filter_map(|i| {
            let t = (serial + i as u64) as f64 / 8_000.0;
            let v = (t * std::f64::consts::TAU * 440.0).sin() as f32 * 0.25;
            stream.sample(Sample {
                stereo: [v, -v * 0.5],
                note,
                epoch,
                serial: serial + i as u64,
            })
        })
        .collect()
}

#[test]
fn every_four_cycle_window_is_consumed_without_repaint_clock_or_amplitude_scaling() {
    let mut stream = Stream::new(8_000);
    let width = crate::scope_trigger::width(8_000, 69);
    let first = feed(&mut stream, 1, 69, 0, width - 1);
    assert!(first.is_empty());
    let frames = feed(&mut stream, 1, 69, (width - 1) as u64, width * 3 + 1);
    assert_eq!(frames.len(), 4);
    for (frame_id, frame) in frames.iter().enumerate() {
        assert_eq!(frame.samples.len(), width);
        for (i, v) in frame.samples.iter().enumerate() {
            let expected = ((frame_id * width + i) as f64 / 8_000.0 * std::f64::consts::TAU * 440.0)
                .sin() as f32
                * 0.25;
            assert_eq!(*v, [expected, -expected * 0.5]);
        }
    }
}

#[test]
fn note_change_discards_partial_tail_and_gap_discards_phase_reference() {
    let mut stream = Stream::new(8_000);
    let width = crate::scope_trigger::width(8_000, 69);
    feed(&mut stream, 1, 69, 0, width + 7);
    let next_width = crate::scope_trigger::width(8_000, 72);
    assert!(feed(&mut stream, 2, 72, (width + 7) as u64, next_width - 1).is_empty());
    assert_eq!(
        feed(&mut stream, 2, 72, (width + next_width + 6) as u64, 1).len(),
        1
    );
    let after_gap = feed(&mut stream, 2, 72, 2_000, next_width);
    assert_eq!(after_gap.len(), 1);
    assert_eq!(after_gap[0].shift_samples, 0.0);
}
