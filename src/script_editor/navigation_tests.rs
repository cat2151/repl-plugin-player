use super::{
    tests::{cursor, frame, page_key},
    *,
};

const CELL_KEYS: Modifiers = Modifiers {
    ctrl: true,
    alt: true,
    ..Modifiers::NONE
};

fn position(text: &str, needle: &str) -> usize {
    text[..text.find(needle).unwrap()].chars().count()
}

#[test]
fn cell_shortcuts_follow_markers_clear_selection_and_do_not_run() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "猫😀();\r\n// %% first\r\none();\r\n  // %% empty\r\n// %% last".to_owned();
    let before = code.clone();
    let first = position(&code, "// %% first");
    let empty = position(&code, "  // %% empty");
    let last = position(&code, "// %% last");
    frame(&ctx, &mut editor, &mut code, vec![], false);
    cursor(&ctx, 2, 2);
    for (key, expected) in [
        (Key::CloseBracket, first),
        (Key::CloseBracket, empty),
        (Key::CloseBracket, last),
        (Key::CloseBracket, last),
        (Key::OpenBracket, empty),
        (Key::OpenBracket, first),
        (Key::OpenBracket, 0),
        (Key::OpenBracket, 0),
    ] {
        assert!(frame(
            &ctx,
            &mut editor,
            &mut code,
            page_key(key, CELL_KEYS),
            false
        )
        .is_none());
        assert_eq!((editor.primary, editor.secondary), (expected, expected));
    }
    // VS Code uses the sorted selection start, including reversed selections.
    for (a, b) in [(first + 3, last + 2), (last + 2, first + 3)] {
        cursor(&ctx, a, b);
        frame(
            &ctx,
            &mut editor,
            &mut code,
            page_key(Key::CloseBracket, CELL_KEYS),
            true,
        );
        assert_eq!((editor.primary, editor.secondary), (empty, empty));
    }
    assert_eq!(code, before);
}

#[test]
fn cell_shortcuts_require_focus_and_modifiers_and_do_not_wrap() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code =
        "// %% first\nx();\n// %%% not a cell\nvar s = '// %%';\n// %% last\ny();".to_owned();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 0, 0);
    for modifiers in [
        Modifiers::NONE,
        Modifiers::CTRL,
        Modifiers::ALT,
        CELL_KEYS | Modifiers::SHIFT,
    ] {
        frame(
            &ctx,
            &mut editor,
            &mut code,
            page_key(Key::CloseBracket, modifiers),
            true,
        );
        assert_eq!(editor.primary, 0);
    }
    ctx.memory_mut(|m| m.surrender_focus(egui::Id::unique("script_text")));
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::CloseBracket, CELL_KEYS),
        true,
    );
    let state = egui::TextEdit::load_state(&ctx, egui::Id::unique("script_text")).unwrap();
    assert_eq!(state.cursor.char_range().unwrap().primary.index.0, 0);
    cursor(&ctx, 1, 1);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::CloseBracket, CELL_KEYS),
        true,
    );
    assert_eq!(editor.primary, position(&code, "// %% last"));
    for text in ["", "one();", "// %% only\none();"] {
        code = text.into();
        cursor(&ctx, 0, 0);
        for key in [Key::OpenBracket, Key::CloseBracket] {
            frame(&ctx, &mut editor, &mut code, page_key(key, CELL_KEYS), true);
            assert_eq!(editor.primary, 0);
            assert_eq!(code, text);
        }
    }
}

#[test]
fn cell_navigation_scrolls_to_the_destination() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = format!("// %% first\n{}// %% second\ny();", "x();\n".repeat(120));
    let expected = position(&code, "// %% second");
    let mut offset = 0.0;
    for step in 0..5 {
        if step == 1 {
            cursor(&ctx, 0, 0);
        }
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 400.0),
                )),
                time: Some(step as f64),
                events: if step == 1 {
                    page_key(Key::CloseBracket, CELL_KEYS)
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    editor.show(ui, &mut code, true);
                    offset = egui::scroll_area::State::load(
                        ui.ctx(),
                        ui.make_persistent_id(egui::IdSalt::new("script_scroll")),
                    )
                    .unwrap()
                    .clamped_offset()
                    .y;
                });
            },
        )
        .drop_without_applying_deltas();
    }
    assert_eq!((editor.primary, editor.secondary), (expected, expected));
    assert!(offset > 1000.0, "destination must be revealed: {offset}");
}
