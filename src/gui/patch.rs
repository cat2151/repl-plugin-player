//! The patch last accepted while a script browsed becomes a cell after the script.
use super::Editor;
use crate::{patch_browse, snippet_insert};
use eframe::egui;

impl Editor {
    /// Inserted after the caret's cell, which is the one that browsed; not run.
    pub(super) fn write_browsed_cell(&mut self, ctx: &egui::Context) {
        if let Some(snippet) = patch_browse::take_cell() {
            self.script_editor
                .write_cell(ctx, &mut self.code, |code, caret| {
                    snippet_insert::insert(code, caret, &snippet)
                });
        }
    }
}
