use super::*;

#[test]
fn escape_returns_typing_to_the_editor_caret() {
    let ctx = egui::Context::default();
    let (mut editor, mut code) = open_snippets(&ctx);
    let script = egui::Id::unique("script_text");
    let mut state = egui::TextEdit::load_state(&ctx, script).unwrap();
    let caret = egui::text::CCursor::new(3);
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(caret)));
    state.store(&ctx, script);
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    let escape = key(egui::Key::Escape, egui::Modifiers::NONE);
    editor_frame(&ctx, &mut editor, &mut code, vec![escape]);
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    editor_frame(&ctx, &mut editor, &mut code, vec![Event::Text("X".into())]);
    assert_eq!(code, "letX a = 1;\n");
}

#[test]
fn escape_closes_snippets_from_filter_and_from_code() {
    let ctx = egui::Context::default();
    let escape = key(egui::Key::Escape, egui::Modifiers::NONE);
    let (mut editor, mut code) = open_snippets(&ctx);
    editor_frame(&ctx, &mut editor, &mut code, vec![escape.clone()]);
    assert!(!editor.snippets().open, "Escape in the filter must close");

    let (mut editor, mut code) = open_snippets(&ctx);
    let script = egui::Id::unique("script_text");
    ctx.memory_mut(|memory| memory.request_focus(script));
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    editor_frame(&ctx, &mut editor, &mut code, vec![escape]);
    assert!(
        !editor.snippets().open,
        "Escape with the code focused must close"
    );
}

#[test]
fn escape_closes_a_modal_above_snippets_first() {
    let ctx = egui::Context::default();
    let mut panel = SnippetsPanel {
        open: true,
        ..Default::default()
    };
    let mut modal = true;
    let run = |panel: &mut SnippetsPanel, modal: &mut bool, events: Vec<Event>| {
        let output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 500.0))),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    panel.show(ui, "", 0, true);
                });
                // Drawn after the panel, as gui.rs draws help and settings.
                if *modal
                    && egui::Modal::new(egui::Id::unique("test_modal"))
                        .show(ui.ctx(), |ui| ui.label("modal"))
                        .should_close()
                {
                    *modal = false;
                }
            },
        );
        output.drop_without_applying_deltas();
    };
    run(&mut panel, &mut modal, vec![]);
    run(&mut panel, &mut modal, vec![]);
    let escape = || vec![key(egui::Key::Escape, egui::Modifiers::NONE)];
    run(&mut panel, &mut modal, escape());
    assert!(!modal, "the modal must close");
    assert!(panel.open, "snippets must stay open under the modal");
    run(&mut panel, &mut modal, vec![]);
    run(&mut panel, &mut modal, escape());
    assert!(!panel.open);
}
