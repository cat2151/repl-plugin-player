use crate::execution;
use eframe::egui::{
    self,
    text::{CCursor, CCursorRange},
    Event,
};

mod key_mapping;

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
    let mut range = state.cursor.char_range()?;
    let mut target = None;
    ui.input_mut(|input| {
        input.events.retain(|event| {
            if let Event::Key {
                key,
                physical_key,
                modifiers,
                pressed: true,
                ..
            } = event
            {
                if modifiers.ctrl && modifiers.alt && !modifiers.shift && !modifiers.mac_cmd {
                    let next = match key_mapping::direction(*key, *physical_key) {
                        Some(next) => next,
                        None => return true,
                    };
                    let start = range.primary.index.0.min(range.secondary.index.0);
                    if let Some(position) = adjacent(text, start, next) {
                        range = CCursorRange::one(CCursor::new(position));
                        target = Some(position);
                    }
                    return false;
                }
            }
            true
        });
    });
    if target.is_some() {
        state.cursor.set_char_range(Some(range));
        state.store(ui.ctx(), id);
    }
    target
}
