use super::*;

/// Same order as gui.rs: the Tab handler runs before any widget of the pass.
fn app_frame(
    ctx: &egui::Context,
    editor: &mut crate::script_editor::ScriptEditor,
    code: &mut String,
    events: Vec<Event>,
) {
    let output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 500.0))),
            events,
            ..Default::default()
        },
        |ui| {
            editor.tab_to_editor(ui.ctx());
            egui::CentralPanel::default().show(ui, |ui| {
                assert!(editor.show(ui, code, true).is_none());
            });
        },
    );
    output.drop_without_applying_deltas();
}

fn tab() -> Event {
    key(egui::Key::Tab, egui::Modifiers::NONE)
}

fn focused(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.memory(|memory| memory.focused())
}

#[test]
fn tab_outside_the_editor_returns_to_it_instead_of_walking_buttons() {
    let ctx = egui::Context::default();
    let mut editor = crate::script_editor::ScriptEditor::default();
    let mut code = "let a = 1;\n".to_string();
    let script = egui::Id::unique("script_text");
    // Without the handler, egui's Tab walks onto a button.
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    editor_frame(&ctx, &mut editor, &mut code, vec![tab()]);
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    let button = focused(&ctx);
    assert!(button.is_some() && button != Some(script), "{button:?}");

    app_frame(&ctx, &mut editor, &mut code, vec![tab()]);
    app_frame(&ctx, &mut editor, &mut code, vec![]);
    assert_eq!(focused(&ctx), Some(script));
    assert_eq!(
        code, "let a = 1;\n",
        "the Tab that moved focus is not typed"
    );

    app_frame(&ctx, &mut editor, &mut code, vec![tab()]);
    assert_ne!(code, "let a = 1;\n", "inside the editor Tab indents");
    assert_eq!(focused(&ctx), Some(script));
}

#[test]
fn tab_in_the_snippets_filter_stays_there() {
    let ctx = egui::Context::default();
    let (mut editor, mut code) = open_snippets(&ctx);
    let filter = egui::Id::unique("snippet_filter");
    app_frame(&ctx, &mut editor, &mut code, vec![tab()]);
    app_frame(&ctx, &mut editor, &mut code, vec![]);
    assert_eq!(focused(&ctx), Some(filter));
    app_frame(&ctx, &mut editor, &mut code, vec![Event::Text("a".into())]);
    assert_eq!(editor.snippets().filter, "a");
    assert_eq!(code, "let a = 1;\n");
}

#[test]
fn shift_tab_outside_the_editor_also_returns_to_it() {
    let ctx = egui::Context::default();
    let mut editor = crate::script_editor::ScriptEditor::default();
    let mut code = "let a = 1;\n".to_string();
    let script = egui::Id::unique("script_text");
    app_frame(&ctx, &mut editor, &mut code, vec![]);
    ctx.memory_mut(|memory| memory.surrender_focus(script));
    let shift_tab = key(egui::Key::Tab, egui::Modifiers::SHIFT);
    app_frame(&ctx, &mut editor, &mut code, vec![shift_tab]);
    app_frame(&ctx, &mut editor, &mut code, vec![]);
    assert_eq!(focused(&ctx), Some(script));
    assert_eq!(code, "let a = 1;\n");
}
