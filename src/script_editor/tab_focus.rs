//! Tab never walks focus between widgets: inside the editor it is indentation,
//! anywhere else it is swallowed. The `focus_editor` key brings the caret back.
use crate::keybinds::{self, Command};
use eframe::egui::{self, Key, Modifiers};

/// Runs before any widget of the pass. With `overlay` (the Snippets window or
/// a modal on top) the key is only swallowed, so focus never lands under it.
pub(super) fn handle(ctx: &egui::Context, overlay: bool) {
    let editor = egui::Id::unique("script_text");
    if ctx.memory(|memory| memory.has_focus(editor)) {
        return;
    }
    let (back, tab) = ctx.input_mut(|input| {
        let back = keybinds::consume_or_shifted(input, Command::FocusEditor);
        let tab = input.consume_key(Modifiers::NONE, Key::Tab)
            | input.consume_key(Modifiers::SHIFT, Key::Tab);
        (back, tab)
    });
    if !back && !tab {
        return;
    }
    let overlay = overlay || ctx.memory(|memory| memory.top_modal_layer()).is_some();
    ctx.memory_mut(|memory| {
        memory.move_focus(egui::FocusDirection::None);
        if back && !overlay {
            memory.request_focus(editor);
        }
    });
}
