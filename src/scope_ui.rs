//! Drawing/normalization adapted from cat-plugin-player 24775301b6d9f367c88c93583e4c0bf7d91cfb97.
use crate::scope;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, scope: Option<&scope::Scope>) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 100.0),
        egui::Sense::hover(),
    );
    let frame = scope.and_then(scope::Scope::display);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(15, 20, 24));
    painter.line_segment(
        [
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ],
        egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
    );
    if let Some(frame) = frame {
        let peak = display_peak(&frame.samples);
        let x = |i: usize| {
            rect.left()
                + rect.width()
                    * (i as f64 / (frame.samples.len() - 1) as f64
                        + frame.shift_samples / frame.samples.len() as f64)
                        as f32
        };
        for (ch, color) in [egui::Color32::LIGHT_GREEN, egui::Color32::LIGHT_BLUE]
            .into_iter()
            .enumerate()
        {
            let points = frame
                .samples
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    egui::pos2(
                        x(i),
                        rect.center().y - normalized_amplitude(s[ch], peak) * rect.height() * 0.4,
                    )
                })
                .collect();
            painter.add(egui::Shape::line(points, egui::Stroke::new(1.0, color)));
        }
    }
}

fn display_peak(samples: &[[f32; 2]]) -> f32 {
    samples
        .iter()
        .flatten()
        .copied()
        .filter(|s| s.is_finite())
        .map(f32::abs)
        .fold(0.0, f32::max)
}

fn normalized_amplitude(sample: f32, peak: f32) -> f32 {
    if peak > 0.0 && sample.is_finite() {
        sample / peak
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_stereo_uses_shared_peak_and_preserves_balance() {
        let samples = [[-0.01, 0.005], [0.01, -0.0025]];
        let peak = display_peak(&samples);
        assert_eq!(normalized_amplitude(samples[0][0], peak), -1.0);
        assert_eq!(normalized_amplitude(samples[1][0], peak), 1.0);
        assert_eq!(normalized_amplitude(samples[0][1], peak), 0.5);
        assert_eq!(normalized_amplitude(samples[1][1], peak), -0.25);
    }

    #[test]
    fn silence_and_non_finite_samples_stay_on_center_line() {
        let samples = [[0.0, f32::NAN], [f32::INFINITY, f32::NEG_INFINITY]];
        let peak = display_peak(&samples);
        assert_eq!(peak, 0.0);
        for sample in samples.iter().flatten() {
            assert_eq!(normalized_amplitude(*sample, peak), 0.0);
        }
    }

    #[test]
    fn over_full_scale_and_tiny_peaks_are_normalized_without_clipping() {
        for peak in [2.0, f32::MIN_POSITIVE / 2.0] {
            let samples = [[-peak, peak / 2.0], [peak, f32::NAN]];
            let measured = display_peak(&samples);
            assert_eq!(normalized_amplitude(-peak, measured), -1.0);
            assert_eq!(normalized_amplitude(peak / 2.0, measured), 0.5);
            assert_eq!(normalized_amplitude(f32::NAN, measured), 0.0);
        }
    }
}
