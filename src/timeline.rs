mod notes;
mod paint;
pub(crate) mod projection;
pub(crate) mod regression;

use eframe::egui;

pub fn show(ui: &mut egui::Ui, view: &projection::View) {
    paint::show(ui, view);
}
