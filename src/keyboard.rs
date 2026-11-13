//! The keyboard display, drawn by `repl_keyboard.dll` (`extensions/keyboard`).
use eframe::egui;
use libloading::Library;
use std::sync::OnceLock;

type Show = fn(&mut egui::Ui, &[u8], Option<(u8, u8)>, &str);

struct Keyboard {
    /// Never dropped, so the DLL stays loaded for the whole process.
    _library: Library,
    show: Show,
}

fn load() -> Result<Keyboard, String> {
    let path = crate::resources::keyboard()?;
    // SAFETY: The DLL is built in this workspace with the same `egui`, so the Rust
    // signature of `repl_keyboard_show` matches `Show`.
    unsafe {
        let library = Library::new(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let show = *library
            .get::<Show>(b"repl_keyboard_show\0")
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Keyboard {
            _library: library,
            show,
        })
    }
}

/// `range` is the lowest and highest pitch in the timeline. When the DLL cannot be
/// loaded, its path and the reason take the keyboard's place.
pub fn show(ui: &mut egui::Ui, active: &[u8], range: Option<(u8, u8)>) {
    static KEYBOARD: OnceLock<Result<Keyboard, String>> = OnceLock::new();
    match KEYBOARD.get_or_init(load) {
        Ok(keyboard) => (keyboard.show)(ui, active, range, crate::i18n::t("m034")),
        Err(error) => {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}

#[cfg(test)]
mod tests;
