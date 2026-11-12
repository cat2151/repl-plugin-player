use eframe::egui;

/// Every run's result, readable and selectable in full, like a browser's developer
/// console. Always at the bottom of the right pane; its top edge drags to resize it.
pub(super) fn show(ui: &mut egui::Ui, results: &[String]) {
    egui::Panel::bottom("console")
        .resizable(true)
        .default_size(160.0)
        .show(ui, |ui| {
            ui.heading("Console");
            let text = results.join("\n\n");
            // A `&str` buffer keeps the text read-only while still selectable and copyable.
            let mut view = text.as_str();
            egui::ScrollArea::both()
                .stick_to_bottom(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut view)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
        });
}
