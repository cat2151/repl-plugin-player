mod notes;
mod paint;
pub(crate) mod projection;
pub(crate) mod regression;

use eframe::egui;
pub use notes::notes;
use serde_json::Value;

pub fn show(ui: &mut egui::Ui, snapshot: &Value, view: &projection::View) {
    paint::show(ui, view);
    egui::CollapsingHeader::new(crate::i18n::t("m042"))
        .id_salt("timeline_raw")
        .show(ui, |ui| {
            egui::ScrollArea::both().max_height(200.0).show(ui, |ui| {
                ui.monospace(serde_json::to_string_pretty(snapshot).unwrap_or_default());
            });
        });
}
