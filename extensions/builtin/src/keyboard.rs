//! The keyboard display.
use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind};
use repl_extension_api::Frame;

fn hover(language: &str) -> &'static str {
    match language {
        "ja" => "全プラグインへの入力です。リリース音やサステインは表示しません。表示専用です。",
        _ => "Input to every plugin · Release tails and sustain are not shown. Display only.",
    }
}

/// The keyboard spans exactly `frame.range`, the lowest and highest pitch in the timeline.
pub(crate) fn show(ui: &mut egui::Ui, frame: &Frame) {
    let (active, range) = (frame.active_notes, frame.range);
    // The height never depends on the notes, so nothing below the keyboard moves.
    let (row, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().min(350.0), 82.0),
        Sense::hover(),
    );
    response.on_hover_text(hover(frame.language));
    let Some((first, last)) = range.map(widen) else {
        return;
    };
    let left = span(first).0;
    let units = span(last).1 - left;
    // Narrow ranges keep piano-like key proportions instead of stretching to the full row.
    let rect = Rect::from_min_size(
        row.min,
        egui::vec2((units * 40.0).clamp(120.0, row.width()), row.height()),
    );
    let scale = rect.width() / units;
    let painter = ui.painter_at(rect);
    // White keys first; black keys overlap their neighbors just as on a piano.
    for is_black in [false, true] {
        for pitch in first..=last {
            if black(pitch) != is_black {
                continue;
            }
            let (start, end) = span(pitch);
            let key = Rect::from_min_max(
                egui::pos2(rect.left() + (start - left) * scale, rect.top()),
                egui::pos2(
                    rect.left() + (end - left) * scale,
                    rect.top() + rect.height() * if is_black { 0.60 } else { 1.0 },
                ),
            );
            let fill = match (active.contains(&pitch), is_black) {
                (true, false) => Color32::from_rgb(60, 190, 150),
                (true, true) => Color32::from_rgb(35, 155, 120),
                (false, false) => Color32::from_gray(225),
                (false, true) => Color32::from_gray(35),
            };
            painter.rect_filled(key, 2.0, fill);
            if !is_black {
                painter.rect_stroke(
                    key,
                    2.0,
                    Stroke::new(1.0_f32, Color32::from_gray(70)),
                    StrokeKind::Inside,
                );
            }
        }
    }
}

/// At least one octave around the notes, with white keys at both edges.
fn widen((first, last): (u8, u8)) -> (u8, u8) {
    let missing = 11u8.saturating_sub(last - first);
    let mut low = first.saturating_sub(missing / 2);
    let mut high = (low + 11).max(last).min(127);
    low = low.min(high - 11);
    while black(low) {
        low -= 1;
    }
    while black(high) {
        high += 1;
    }
    (low, high)
}

fn black(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

/// Diatonic coordinates keep white/black key geometry, even when the range starts on a black key.
fn span(pitch: u8) -> (f32, f32) {
    let white = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6][usize::from(pitch % 12)];
    let x = f32::from(pitch / 12) * 7.0 + white as f32;
    if black(pitch) {
        (x + 0.7, x + 1.3)
    } else {
        (x, x + 1.0)
    }
}
