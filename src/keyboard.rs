use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind};

/// `range` is the lowest and highest pitch in the timeline; the keyboard spans exactly that.
pub fn show(ui: &mut egui::Ui, notes: Result<&[u8], &str>, range: Option<(u8, u8)>) {
    ui.heading(crate::i18n::t("m033"));
    match notes {
        Ok(pitches) => {
            ui.label(crate::i18n::render(
                "f004",
                &[("arg0", numbers(pitches).to_string())],
            ));
        }
        Err(message) => {
            ui.weak(crate::i18n::render(
                "f006",
                &[("message", message.to_string())],
            ));
        }
    }
    // The height never depends on the notes, so nothing below the keyboard moves.
    let (row, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().min(350.0), 82.0),
        Sense::hover(),
    );
    response.on_hover_text(crate::i18n::t("m034"));
    let Some((first, last)) = range.map(widen) else {
        return;
    };
    let active = notes.unwrap_or_default();
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

fn numbers(notes: &[u8]) -> String {
    if notes.is_empty() {
        "—".into()
    } else {
        notes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::widen;

    #[test]
    fn narrow_ranges_widen_to_an_octave_with_white_edges() {
        assert_eq!(widen((61, 63)), (57, 69));
        assert_eq!(widen((60, 71)), (60, 71));
        assert_eq!(widen((36, 84)), (36, 84));
        assert_eq!(widen((0, 0)), (0, 11));
        assert_eq!(widen((127, 127)), (115, 127));
        assert_eq!(widen((61, 61)), (55, 67));
    }
}
