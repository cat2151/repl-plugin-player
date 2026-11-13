//! The extensions this repository holds. The app and this DLL are built in one
//! workspace, so both sides share the same `egui` and pass `egui::Ui` directly.
use eframe::egui;
use repl_extension_api::{Extension, List};

mod keyboard;

const _: List = repl_extensions;

#[no_mangle]
pub fn repl_extensions() -> &'static [Extension] {
    &[Extension {
        name: "keyboard",
        show: keyboard::show,
    }]
}

/// A tall list in a `ScrollArea` with the id salt `"repl_keyboard_probe"`. Only the app's
/// tests draw it, to see that the app reads the scroll state this DLL stores.
#[no_mangle]
pub fn repl_keyboard_probe_scroll(ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .id_salt("repl_keyboard_probe")
        .max_height(100.0)
        .show(ui, |ui| {
            for row in 0..100 {
                ui.label(row.to_string());
            }
        });
}
