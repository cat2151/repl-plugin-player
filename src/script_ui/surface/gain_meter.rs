use crate::i18n::{render, t};
use eframe::egui::{self, Color32, Rect};
use std::time::Duration;

const WIDTH: f32 = 48.0;
const HEIGHT: f32 = 5.0;
const GAP: f32 = 2.0;
const RANGE_DB: f32 = 18.0;
pub(super) const BOOST: Color32 = Color32::from_rgb(95, 180, 240);
pub(super) const LIMIT: Color32 = Color32::from_rgb(238, 128, 85);

pub(super) fn draw(ui: &mut egui::Ui, checked: bool, instance: Option<i32>) {
    let (pad, master) = if checked {
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        crate::script_api::pad_gain_meter(instance)
    } else {
        (None, None)
    };
    let width = ui.available_width().clamp(0.0, WIDTH);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, 2.0 * HEIGHT + GAP), egui::Sense::hover());
    let db = |gain: f32| 20.0 * gain.max(f32::MIN_POSITIVE).log10();
    let boost = pad.map(db);
    let reduction = master.map(|gain| {
        let db = db(gain);
        if db > -0.05 {
            0.0
        } else {
            db
        }
    });
    let top = Rect::from_min_size(rect.min, egui::vec2(width, HEIGHT));
    let bottom = top.translate(egui::vec2(0.0, HEIGHT + GAP));
    bar(ui, top, boost, BOOST);
    let limit_color = if reduction.is_some_and(|db| db < 0.0) {
        LIMIT
    } else {
        ui.visuals().weak_text_color()
    };
    bar(ui, bottom, reduction.map(|db| -db), limit_color);
    response.on_hover_ui(|ui| {
        if !checked {
            ui.label(t("pad_gain_off"));
            return;
        }
        let pad = match boost {
            Some(db) => render("pad_gain_db", &[("db", format!("{db:+.1}"))]),
            None if instance.is_none() => t("pad_gain_no_selection").into(),
            None => t("pad_gain_no_audio").into(),
        };
        let master = reduction.map_or_else(
            || t("pad_gain_limit_no_audio").into(),
            |db| render("pad_gain_limit_db", &[("db", format!("{db:.1}"))]),
        );
        ui.label(pad);
        ui.label(master);
    });
}

fn bar(ui: &egui::Ui, rect: Rect, db: Option<f32>, color: Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, ui.visuals().extreme_bg_color);
    if let Some(db) = db {
        let fill = (rect.width() * (db / RANGE_DB).clamp(0.0, 1.0))
            .max(2.0)
            .min(rect.width());
        painter.rect_filled(
            Rect::from_min_size(rect.min, egui::vec2(fill, rect.height())),
            1.0,
            color,
        );
    }
}
