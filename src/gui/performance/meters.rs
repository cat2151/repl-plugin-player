use eframe::egui::{self, Color32, Rect, Vec2};

const WIDTH: f32 = 90.0;
const BAR_HEIGHT: f32 = 5.0;
const GAP: f32 = 2.0;

fn bar(ui: &egui::Ui, rect: Rect, fraction: Option<f32>, danger: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, ui.visuals().extreme_bg_color);
    let threshold = rect.left() + rect.width() * 0.8;
    if danger {
        painter.rect_filled(
            Rect::from_min_max(egui::pos2(threshold, rect.top()), rect.max),
            0.0,
            Color32::from_rgb(75, 25, 25),
        );
    }
    if let Some(fraction) = fraction {
        let right = rect.left() + rect.width() * fraction.clamp(0.0, 1.0);
        if right > rect.left() {
            painter.rect_filled(
                Rect::from_min_max(
                    rect.min,
                    egui::pos2(
                        right.min(if danger { threshold } else { rect.right() }),
                        rect.bottom(),
                    ),
                ),
                0.0,
                Color32::from_rgb(90, 185, 135),
            );
        }
        if danger && fraction >= 0.8 {
            // Include a visible red edge even at exactly 80%.
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(threshold - 1.0, rect.top()),
                    egui::pos2(right.max(threshold + 1.0).min(rect.right()), rect.bottom()),
                ),
                0.0,
                Color32::LIGHT_RED,
            );
        }
    }
}

fn rows(rect: Rect, row_height: f32) -> [Rect; 2] {
    [0, 1].map(|row| {
        let top = rect.top() + row as f32 * (row_height + GAP) + (row_height - BAR_HEIGHT) * 0.5;
        Rect::from_min_size(egui::pos2(rect.left(), top), Vec2::new(WIDTH, BAR_HEIGHT))
    })
}

pub(super) fn notes(ui: &mut egui::Ui, values: [Option<u64>; 2]) {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let texts = values.map(|value| value.map_or_else(|| " —".into(), |n| format!("{n:>2}")));
    let galleys = texts.map(|text| {
        ui.painter()
            .layout_no_wrap(text, font.clone(), ui.visuals().text_color())
    });
    let row_height = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max);
    let number_width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(WIDTH + 4.0 + number_width, row_height * 2.0 + GAP),
        egui::Sense::hover(),
    );
    for (row, bar_rect) in rows(rect, row_height).into_iter().enumerate() {
        bar(ui, bar_rect, values[row].map(|n| n as f32 / 99.0), false);
        let galley = galleys[row].clone();
        ui.painter().galley(
            egui::pos2(
                rect.right() - galley.size().x,
                rect.top() + row as f32 * (row_height + GAP),
            ),
            galley,
            ui.visuals().text_color(),
        );
    }
    response.on_hover_text(crate::i18n::t("performance_notes_help"));
}

pub(super) fn load(ui: &mut egui::Ui, values: Option<[f64; 2]>) {
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(WIDTH, row_height * 2.0 + GAP),
        egui::Sense::hover(),
    );
    for (row, bar_rect) in rows(rect, row_height).into_iter().enumerate() {
        bar(ui, bar_rect, values.map(|v| v[row] as f32 / 100.0), true);
    }
    response.on_hover_text(crate::i18n::t("performance_load_help"));
}
