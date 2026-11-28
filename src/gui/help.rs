//! The keybind overlay: toggled by the `help` key of `keybinds.toml`, also closed by
//! `Escape`. The Snippets window has its own page.
use crate::keybinds::Command;
use eframe::egui::{self, Color32};

const RUN: Color32 = Color32::from_rgb(110, 200, 120);
const MOVE: Color32 = Color32::from_rgb(110, 170, 240);
const SNIPPETS: Color32 = Color32::from_rgb(230, 170, 90);
const SCREEN: Color32 = Color32::from_rgb(200, 140, 230);

/// A key from `keybinds.toml`, or a pair of them shown as `A / B`.
#[derive(Debug, PartialEq)]
enum Keys {
    Bound(Command),
    Pair(Command, Command),
}

impl Keys {
    fn text(&self) -> String {
        match self {
            Self::Bound(command) => crate::keybinds::text(*command).into(),
            Self::Pair(first, second) => format!(
                "{} / {}",
                crate::keybinds::text(*first),
                crate::keybinds::text(*second)
            ),
        }
    }
}

type Page = &'static [(Color32, Keys, &'static str)];

/// Keys share their group's color; the words beside them come from the locale.
const EDITOR: Page = &[
    (RUN, Keys::Bound(Command::RunCell), "help_run_cell"),
    (RUN, Keys::Bound(Command::RunLine), "help_run_line"),
    (
        MOVE,
        Keys::Bound(Command::PreviousCell),
        "help_previous_cell",
    ),
    (MOVE, Keys::Bound(Command::NextCell), "help_next_cell"),
    (
        MOVE,
        Keys::Pair(Command::PageUp, Command::PageDown),
        "help_page",
    ),
    (MOVE, Keys::Bound(Command::Fold), "help_fold"),
    (MOVE, Keys::Bound(Command::FocusEditor), "help_focus_editor"),
    (
        SCREEN,
        Keys::Bound(Command::CloseWindow),
        "help_close_window",
    ),
    (SNIPPETS, Keys::Bound(Command::Snippets), "help_snippets"),
    (SCREEN, Keys::Bound(Command::Settings), "help_settings"),
    (SCREEN, Keys::Bound(Command::Help), "help_help"),
];

/// Shown instead of `EDITOR` while the Snippets window is open.
const SNIPPET_WINDOW: Page = &[
    (
        SNIPPETS,
        Keys::Bound(Command::SnippetFilter),
        "help_snippet_filter",
    ),
    (
        MOVE,
        Keys::Pair(Command::SnippetUp, Command::SnippetDown),
        "help_snippet_select",
    ),
    (
        MOVE,
        Keys::Pair(Command::SnippetPageUp, Command::SnippetPageDown),
        "help_page",
    ),
    (RUN, Keys::Bound(Command::SnippetRun), "help_snippet_run"),
    (
        SCREEN,
        Keys::Bound(Command::SnippetClose),
        "help_snippet_close",
    ),
    (SCREEN, Keys::Bound(Command::Help), "help_help"),
];

#[derive(Default)]
pub(super) struct Help {
    open: Option<Page>,
}

impl Help {
    /// The page is chosen when the key opens it, from the window that has the user's attention.
    pub fn shortcut(&mut self, ctx: &egui::Context, snippets_open: bool) {
        if ctx.input_mut(|input| crate::keybinds::consume(input, crate::keybinds::Command::Help)) {
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
                        ui.label(egui::RichText::new(keys.text()).monospace().color(*color));
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
    use egui::{Event, Key, Modifiers, Pos2, Rect};

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
