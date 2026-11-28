use crate::{
    execution,
    keybinds::{self, Command},
};
use eframe::egui::{
    self,
    text::{CCursor, CCursorRange},
};

// VS Code Jupyter codewatcher.ts at 0a68e24f4305c3248a47f7c7abb87c13f1e5c3c7:
// gotoNextCell/gotoPreviousCell use selection.start; advanceToRange collapses
// the selection at the adjacent cell's range.start. Keep our // %% grammar.
fn adjacent(text: &str, cursor: usize, next: bool) -> Option<usize> {
    let mut starts = vec![0];
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if offset != 0 && execution::marker(line) {
            starts.push(offset);
        }
        offset += line.chars().count();
    }
    let current = starts
        .partition_point(|start| *start <= cursor)
        .saturating_sub(1);
    let target = if next {
        current.checked_add(1)?
    } else {
        current.checked_sub(1)?
    };
    starts.get(target).copied()
}

pub(super) fn handle(ui: &mut egui::Ui, id: egui::Id, text: &str) -> Option<usize> {
    if !ui.memory(|memory| memory.has_focus(id)) {
        return None;
    }
    let mut state = egui::TextEdit::load_state(ui.ctx(), id)?;
    let range = state.cursor.char_range()?;
    let next = ui.input_mut(|input| {
        if keybinds::consume(input, Command::NextCell) {
            Some(true)
        } else if keybinds::consume(input, Command::PreviousCell) {
            Some(false)
        } else {
            None
        }
    })?;
    let start = range.primary.index.0.min(range.secondary.index.0);
    let target = adjacent(text, start, next)?;
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(target))));
    state.store(ui.ctx(), id);
    Some(target)
}
