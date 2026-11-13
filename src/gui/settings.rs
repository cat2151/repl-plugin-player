//! The settings overlay: registers the external editor, opens `config.toml` in it, and
//! chooses whether the layout is the auto-saved one. Opened and closed by `Ctrl+,`.
use crate::external_editor;
use eframe::egui::{self, Key, Modifiers};
use std::path::PathBuf;

const INPUT: &str = "settings_editor_input";

pub(super) struct Settings {
    open: bool,
    focus_input: bool,
    input: String,
    editor: Option<PathBuf>,
    /// The layout is `layout-auto.toml`'s, else the `[layout]` of `config.toml`'s.
    pub layout_auto: bool,
    /// The radio buttons changed `layout_auto` since the editor last took it.
    layout_switched: bool,
    error: String,
    status: Result<PathBuf, String>,
    config: Result<PathBuf, String>,
}

impl Settings {
    pub fn new(status: Result<PathBuf, String>, config: Result<PathBuf, String>) -> Self {
        let (editor, error) = match &status {
            Ok(path) => external_editor::read(path),
            Err(error) => (None, Some(error.clone())),
        };
        let (layout_auto, layout_error) = match &status {
            Ok(path) => crate::status::layout_auto(path),
            Err(_) => (true, None),
        };
        Self {
            open: false,
            focus_input: false,
            input: editor
                .as_ref()
                .map(|editor| editor.display().to_string())
                .unwrap_or_default(),
            editor,
            layout_auto,
            layout_switched: false,
            error: error.or(layout_error).unwrap_or_default(),
            status,
            config,
        }
    }

    fn toggle(&mut self) {
        self.open = !self.open;
        self.focus_input = self.open;
    }

    /// Consumes `Ctrl+,` before any text editor sees it, so the comma is never typed.
    pub fn shortcut(&mut self, ctx: &egui::Context) {
        let pressed = ctx.input_mut(|input| {
            let pressed = input.consume_key(Modifiers::CTRL, Key::Comma);
            if pressed {
                input
                    .events
                    .retain(|event| !matches!(event, egui::Event::Text(text) if text == ","));
            }
            pressed
        });
        if pressed {
            self.toggle();
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("settings_overlay")).show(ctx, |ui| {
            let mut close = false;
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("settings_editor"));
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.input)
                        .id(egui::Id::unique(INPUT))
                        .desired_width(420.0),
                );
                if std::mem::take(&mut self.focus_input) {
                    input.request_focus();
                }
                let entered = input.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if ui.button(crate::i18n::t("settings_save")).clicked() || entered {
                    self.save();
                }
                close = ui.button("×").clicked();
            });
            if ui.button(crate::i18n::t("settings_open_config")).clicked() {
                self.open_config();
            }
            let before = self.layout_auto;
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("settings_layout"));
                let auto = crate::i18n::t("settings_layout_auto");
                ui.radio_value(&mut self.layout_auto, true, auto);
                let config = crate::i18n::t("settings_layout_config");
                ui.radio_value(&mut self.layout_auto, false, config);
            });
            if self.layout_auto != before {
                self.layout_switched = true;
                self.save_layout_auto();
            }
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &self.error);
            }
            close
        });
        if response.inner || response.should_close() {
            self.open = false;
        }
    }

    fn save(&mut self) {
        let result = self
            .status
            .clone()
            .and_then(|path| external_editor::save(&path, &self.input));
        match result {
            Ok(editor) => {
                self.editor = editor;
                self.error.clear();
            }
            Err(error) => self.error = error,
        }
    }

    /// Whether the radio buttons switched the layout's source since the last call.
    pub fn take_layout_switch(&mut self) -> bool {
        std::mem::take(&mut self.layout_switched)
    }

    fn save_layout_auto(&mut self) {
        let result = (self.status.clone())
            .and_then(|path| crate::status::save_layout_auto(&path, self.layout_auto));
        match result {
            Ok(()) => self.error.clear(),
            Err(error) => self.error = error,
        }
    }

    fn open_config(&mut self) {
        let result = self
            .config
            .clone()
            .and_then(|path| external_editor::open_config(self.editor.as_deref(), &path));
        match result {
            Ok(()) => self.error.clear(),
            Err(error) => self.error = error,
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
