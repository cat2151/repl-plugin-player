//! The settings overlay: chooses the easy style for the next startup, registers the
//! external editor, opens `config.toml` in it, chooses whether the layout is the
//! auto-saved one, and turns extensions on or off for the next startup. Opened and closed
//! by the `settings` key of `keybinds.toml`.
use crate::external_editor;
use eframe::egui::{self, Key};
use std::path::PathBuf;

mod extensions;

const INPUT: &str = "settings_editor_input";

pub(super) struct Settings {
    open: bool,
    focus_input: bool,
    input: String,
    editor: Option<PathBuf>,
    /// Startup opens and runs `easy-init`; read once at startup.
    pub easy: bool,
    /// The layout is `layout-auto.toml`'s, else the `[layout]` of `config.toml`'s.
    pub layout_auto: bool,
    /// The radio buttons changed `layout_auto` since the editor last took it.
    layout_switched: bool,
    extensions: extensions::Chooser,
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
        let (easy, easy_error) = match &status {
            Ok(path) => crate::status::easy(path),
            Err(_) => (true, None),
        };
        let (extensions, extensions_error) = match &status {
            Ok(path) => crate::status::extensions(path),
            Err(_) => (
                crate::status::DEFAULT_EXTENSIONS.map(String::from).to_vec(),
                None,
            ),
        };
        Self {
            open: false,
            focus_input: false,
            input: editor
                .as_ref()
                .map(|editor| editor.display().to_string())
                .unwrap_or_default(),
            editor,
            easy,
            layout_auto,
            layout_switched: false,
            extensions: extensions::Chooser::new(extensions),
            error: (error.or(easy_error).or(layout_error).or(extensions_error)).unwrap_or_default(),
            status,
            config,
        }
    }

    fn toggle(&mut self) {
        self.open = !self.open;
        self.focus_input = self.open;
    }

    /// Consumed before any text editor sees it, so its character is never typed.
    pub fn shortcut(&mut self, ctx: &egui::Context) {
        let pressed = ctx
            .input_mut(|input| crate::keybinds::consume(input, crate::keybinds::Command::Settings));
        if pressed {
            self.toggle();
        }
    }

    /// The `<dll>/<pane>` names on at startup.
    pub fn extensions(&self) -> &[String] {
        &self.extensions.running
    }

    pub fn show(&mut self, ctx: &egui::Context, extensions: &mut crate::extension::Extensions) {
        if !self.open {
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("settings_overlay")).show(ctx, |ui| {
            let mut close = false;
            let easy = self.easy;
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("settings_easy"));
                ui.radio_value(&mut self.easy, true, crate::i18n::t("settings_on"));
                ui.radio_value(&mut self.easy, false, crate::i18n::t("settings_off"));
            });
            if self.easy != easy {
                self.save_easy();
            }
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
            ui.separator();
            if self.extensions.show(ui, extensions) {
                self.save_extensions();
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

    fn save_easy(&mut self) {
        let result =
            (self.status.clone()).and_then(|path| crate::status::save_easy(&path, self.easy));
        match result {
            Ok(()) => self.error.clear(),
            Err(error) => self.error = error,
        }
    }

    fn save_layout_auto(&mut self) {
        let result = (self.status.clone())
            .and_then(|path| crate::status::save_layout_auto(&path, self.layout_auto));
        match result {
            Ok(()) => self.error.clear(),
            Err(error) => self.error = error,
        }
    }

    fn save_extensions(&mut self) {
        let chosen = &self.extensions.chosen;
        let result =
            (self.status.clone()).and_then(|path| crate::status::save_extensions(&path, chosen));
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
