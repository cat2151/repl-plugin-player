//! The keybind overlay: toggled by `F1`, closed by `F1` or `Escape`.
//! The Snippets window has its own page.
use eframe::egui::{self, Color32, Key, Modifiers};

const RUN: Color32 = Color32::from_rgb(110, 200, 120);
const MOVE: Color32 = Color32::from_rgb(110, 170, 240);
const SNIPPETS: Color32 = Color32::from_rgb(230, 170, 90);
const SCREEN: Color32 = Color32::from_rgb(200, 140, 230);

type Page = &'static [(Color32, &'static str, &'static str)];

/// Keys share their group's color; the words beside them come from the locale.
const EDITOR: Page = &[
    (RUN, "Ctrl+Enter", "help_run_cell"),
    (RUN, "Shift+Enter", "help_run_line"),
    (MOVE, "Ctrl+Alt+[", "help_previous_cell"),
    (MOVE, "Ctrl+Alt+]", "help_next_cell"),
    (MOVE, "PageUp / PageDown", "help_page"),
    (MOVE, "Ctrl+.", "help_fold"),
    (SNIPPETS, "Ctrl+Shift+P", "help_snippets"),
    (SCREEN, "Ctrl+,", "help_settings"),
    (SCREEN, "F1", "help_help"),
];

/// Shown instead of `EDITOR` while the Snippets window is open.
const SNIPPET_WINDOW: Page = &[
    (SNIPPETS, "/", "help_snippet_filter"),
    (MOVE, "↑ / ↓", "help_snippet_select"),
    (RUN, "Enter", "help_snippet_run"),
    (SCREEN, "F1", "help_help"),
];

#[derive(Default)]
pub(super) struct Help {
    open: Option<Page>,
}

impl Help {
    /// The page is chosen when F1 opens it, from the window that has the user's attention.
    pub fn shortcut(&mut self, ctx: &egui::Context, snippets_open: bool) {
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F1)) {
            self.open = match self.open {
                Some(_) => None,
                None if snippets_open => Some(SNIPPET_WINDOW),
                None => Some(EDITOR),
            };
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let Some(page) = self.open else {
            return;
        };
        let response = egui::Modal::new(egui::Id::unique("help_overlay")).show(ctx, |ui| {
            egui::Grid::new("help_keys")
                .num_columns(2)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    for (color, keys, word) in page {
                        ui.label(egui::RichText::new(*keys).monospace().color(*color));
                        ui.label(crate::i18n::t(word));
                        ui.end_row();
                    }
                });
        });
        if response.should_close() {
            self.open = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Pos2, Rect};

    fn frame(ctx: &egui::Context, help: &mut Help, key: Option<Key>, snippets_open: bool) {
        let events = key
            .map(|key| Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .into_iter()
            .collect();
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1250.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| {
                help.shortcut(ui.ctx(), snippets_open);
                help.show(ui.ctx());
            },
        )
        .drop_without_applying_deltas();
    }

    #[test]
    fn f1_opens_the_page_of_the_open_window_and_escape_closes() {
        let ctx = egui::Context::default();
        let mut help = Help::default();
        frame(&ctx, &mut help, Some(Key::F1), false);
        assert_eq!(help.open, Some(EDITOR));
        frame(&ctx, &mut help, Some(Key::F1), false);
        assert_eq!(help.open, None);
        frame(&ctx, &mut help, Some(Key::F1), true);
        assert_eq!(help.open, Some(SNIPPET_WINDOW));
        frame(&ctx, &mut help, None, true);
        frame(&ctx, &mut help, Some(Key::Escape), true);
        assert_eq!(help.open, None);
    }

    #[test]
    fn every_word_is_in_the_catalog() {
        for (_, _, word) in EDITOR.iter().chain(SNIPPET_WINDOW) {
            crate::i18n::text(word).unwrap();
        }
    }
}
