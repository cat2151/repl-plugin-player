use super::*;
use crate::timeline::projection::{self, Note};

fn fixture() -> View {
    View {
        tracks: vec![Track {
            id: "fixture".into(),
            index: 0,
            plugins: vec!["Instrument".into()],
            low: 58,
            high: 73,
            clips: vec![Clip {
                label: "Chord".into(),
                start: 1.0,
                end: 4.0,
                muted: false,
                looping: false,
                notes: [60, 64, 67]
                    .into_iter()
                    .map(|pitch| Note {
                        pitch,
                        group: 0,
                        channel: 0,
                        start: 1.0,
                        end: 3.0,
                        open: false,
                    })
                    .collect(),
            }],
        }],
        end: 8.0,
        playhead: 2.0,
        playing: true,
        loop_end: None,
        signatures: vec![projection::Signature {
            start: 0.0,
            numerator: 4,
            denominator: 4,
        }],
    }
}

fn rectangles(shape: &egui::Shape, fill: Color32, output: &mut Vec<Rect>) {
    match shape {
        egui::Shape::Rect(rect) if rect.fill == fill => output.push(rect.rect),
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| rectangles(s, fill, output)),
        _ => {}
    }
}

#[test]
fn chord_notes_use_pitch_rows_and_one_time_axis_at_different_widths() {
    for width in [260.0, 560.0, 1000.0] {
        let ctx = egui::Context::default();
        let view = fixture();
        let mut bars = Vec::new();
        for _ in 0..2 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 500.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| show(ui, &view));
                },
            );
            bars.clear();
            for shape in &output.shapes {
                rectangles(&shape.shape, color(&view.tracks[0].clips[0]), &mut bars);
            }
            output.drop_without_applying_deltas();
        }
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].x_range(), bars[1].x_range());
        assert_eq!(bars[1].x_range(), bars[2].x_range());
        assert!((bars[0].top() - bars[1].top() - 4.0 * SEMITONE).abs() < 0.01);
        assert!((bars[1].top() - bars[2].top() - 3.0 * SEMITONE).abs() < 0.01);
        assert!(bars.iter().all(|r| r.is_finite() && r.width() > 2.0));
    }
}
