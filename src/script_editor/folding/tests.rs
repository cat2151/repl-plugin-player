use super::*;
use crate::script_editor::{
    tests::{cursor, frame, page_key},
    ScriptEditor,
};

#[test]
fn editing_neighbor_preserves_fold_and_navigation_reveals_hidden_body() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "// %% first\n猫😀();\nx();\n// %% next\ny();".to_owned();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 0, 0);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Period, Modifiers::CTRL),
        true,
    );
    let next = cells(&code)[1].hidden.start;
    cursor(&ctx, next + 1, next + 1);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![Event::Text("Z".into())],
        true,
    );
    assert_eq!(editor.folds.closed, BTreeSet::from([0]));
    assert!(code.contains("yZ();"));
    cursor(&ctx, 0, 0);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::ArrowDown, Modifiers::NONE),
        true,
    );
    assert!(editor.folds.closed.is_empty());
    assert!(editor.primary >= "// %% first\n".chars().count());
}

#[test]
fn selection_across_fold_can_be_painted_copied_and_replaced_then_undone() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "// %% first\n猫😀();\nx();\n// %% next\ny();".to_owned();
    let original = code.clone();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 0, 0);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Period, Modifiers::CTRL),
        true,
    );
    cursor(&ctx, code.chars().count(), 0);
    frame(&ctx, &mut editor, &mut code, vec![Event::Copy], true);
    assert_eq!(editor.folds.closed, BTreeSet::from([0]));
    assert_eq!(code, original);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![Event::Paste("new();".into())],
        true,
    );
    assert!(editor.folds.closed.is_empty());
    assert_eq!(code, "new();");
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Z, Modifiers::CTRL | Modifiers::COMMAND),
        true,
    );
    assert_eq!(code, original);
}

#[test]
fn folding_keeps_full_script_execution_and_undo_and_requires_focus() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "// %% 猫\r\none();\r\ntwo();\r\n// %% next\r\nthree();".to_owned();
    let original = code.clone();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 12, 12);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Period, Modifiers::CTRL),
        true,
    );
    assert_eq!(editor.folds.closed, BTreeSet::from([0]));
    assert_eq!((editor.primary, editor.secondary), (0, 0));
    let request = frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Enter, Modifiers::CTRL),
        true,
    )
    .unwrap();
    assert_eq!(request.code, "one();\r\ntwo();\r\n");
    assert_eq!(code, original);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Period, Modifiers::CTRL),
        false,
    );
    assert!(editor.folds.closed.is_empty());
    cursor(&ctx, 10, 10);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![Event::Text("X".into())],
        true,
    );
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Period, Modifiers::CTRL),
        true,
    );
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Z, Modifiers::CTRL | Modifiers::COMMAND),
        true,
    );
    assert_eq!(code, original);
    ctx.memory_mut(|m| m.surrender_focus(egui::Id::unique("script_text")));
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::Period, Modifiers::CTRL),
        true,
    );
    assert!(editor.folds.closed.is_empty());
}

#[test]
fn layout_hides_meshes_and_height_but_preserves_unicode_offsets() {
    let ctx = egui::Context::default();
    let text = "// %% one\r\n猫😀();\r\nx();\r\n// %% two\r\ny();\r\n// %% last\r\nz();";
    let current = cells(text);
    let mut folds = Folding::default();
    folds.sync(text);
    folds.closed.extend([0, current[2].start]);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let full = ui.painter().layout_no_wrap(
            text.into(),
            egui::FontId::monospace(14.0),
            egui::Color32::WHITE,
        );
        let folded = folds.layout(full.clone());
        assert_eq!(folded.job.text, text);
        assert_eq!(folded.rows.len(), full.rows.len());
        assert!(folded.rect.height() < full.rect.height());
        assert!(
            folded.pos_from_cursor(CCursor::new(current[1].start)).top()
                < full.pos_from_cursor(CCursor::new(current[1].start)).top()
        );
        let mut offset = 0;
        for row in &folded.rows {
            if current[0].hidden.contains(&offset) || current[2].hidden.contains(&offset) {
                assert_eq!(row.size.y, 0.0);
                assert!(row.visuals.mesh.vertices.is_empty());
            }
            offset += row.char_count_including_newline().0;
        }
        assert_eq!(offset, text.chars().count());
    })
    .drop_without_applying_deltas();
}

#[test]
fn fold_anchors_follow_edits_before_neighbor_and_headers_without_markers_work() {
    let mut folds = Folding::default();
    let text = "head\nbody\n// %% next\n猫\n// %% last\nend";
    folds.sync(text);
    let second = cells(text)[1].start;
    folds.closed.insert(second);
    folds.sync(&format!("😀{text}"));
    assert_eq!(folds.closed, BTreeSet::from([second + 1]));
    folds.sync("head\nbody\n// deleted\n猫\n// %% last\nend");
    assert!(folds.closed.is_empty());
    assert_eq!(cells("one\ntwo")[0].hidden, 4..7);
    assert!(cells("").is_empty());
    assert!(cells("// %%")[0].hidden.is_empty());
}

#[test]
fn gutter_click_toggles_individual_cells_and_returns_keyboard_focus() {
    let ctx = egui::Context::default();
    let id = egui::Id::unique("script_text");
    let mut folds = Folding::default();
    let mut code = "// %% one\nx();\n// %% two\ny();".to_owned();
    let mut button = egui::Pos2::ZERO;
    for step in 0..5 {
        let events = if step == 1 || step == 3 {
            vec![
                Event::PointerMoved(button),
                Event::PointerButton {
                    pos: button,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]
        } else if step == 2 || step == 4 {
            vec![Event::PointerButton {
                pos: button,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]
        } else {
            vec![]
        };
        ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_space(24.0);
                    let mut output = egui::TextEdit::multiline(&mut code).id(id).show(ui);
                    button = egui::pos2(
                        output.response.rect.left() - 11.0,
                        output.galley_pos.y + output.galley.rows[0].size.y / 2.0,
                    );
                    folds.controls(ui, id, &code, &mut output);
                });
            },
        )
        .drop_without_applying_deltas();
        if step == 2 {
            assert_eq!(folds.closed, BTreeSet::from([0]));
            assert!(ctx.memory(|m| m.has_focus(id)));
        }
    }
    assert!(folds.closed.is_empty());
    assert_eq!(code, "// %% one\nx();\n// %% two\ny();");
}
