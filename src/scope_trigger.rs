//! Sample-domain oscilloscope alignment, independent of audio devices and UI.

pub fn width(sample_rate: u32, note: u8) -> usize {
    let frequency = 440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0);
    (f64::from(sample_rate) / frequency * 4.0).round().max(2.0) as usize
}

pub(crate) fn channel(samples: &[[f32; 2]]) -> usize {
    if samples.iter().any(|s| s[0].abs() > 1e-6) {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_cycles_follow_note_and_octave() {
        assert_eq!(width(48_000, 69), 436);
        assert_eq!(width(48_000, 81), 218);
    }
}
