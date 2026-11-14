//! Tab never walks focus between widgets: inside the editor it is indentation,
//! anywhere else it brings the caret back to the editor.
use eframe::egui::{self, Key, Modifiers};

/// Runs before any widget of the pass. With `overlay` (the Snippets window or
/// a modal on top) Tab is only swallowed, so focus never lands under it.
pub(super) fn handle(ctx: &egui::Context, overlay: bool) {
    let editor = egui::Id::unique("script_text");
    if ctx.memory(|memory| memory.has_focus(editor)) {
        return;
    }
    let tab = ctx.input_mut(|input| {
        input.consume_key(Modifiers::NONE, Key::Tab) | input.consume_key(Modifiers::SHIFT, Key::Tab)
    });
    if !tab {
        return;
    }
    let overlay = overlay || ctx.memory(|memory| memory.top_modal_layer()).is_some();
    ctx.memory_mut(|memory| {
        memory.move_focus(egui::FocusDirection::None);
        if !overlay {
            memory.request_focus(editor);
        }
    });
}
