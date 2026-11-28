mod audio;
mod notes;
mod paint;
mod plugin_names;
pub(crate) mod projection;
pub(crate) mod regression;

use eframe::egui;

const RECORDING_COLOR: egui::Color32 = egui::Color32::from_rgb(242, 73, 85);

pub fn show(ui: &mut egui::Ui, view: &projection::View) {
    paint::show(ui, view);
}
