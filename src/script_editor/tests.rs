use super::*;
use egui::{text::CCursorRange, Event, RawInput};

pub(super) fn frame(
    ctx: &egui::Context,
    editor: &mut ScriptEditor,
    code: &mut String,
    events: Vec<Event>,
    ready: bool,
) -> Option<Request> {
    let mut result = None;
    // Headless UI tests inspect shapes/state and have no renderer for textures.
    ctx.run_ui(
        RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 400.0),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                result = editor.show(ui, code, ready);
            });
        },
    )
    .drop_without_applying_deltas();
    result
}

fn key(modifiers: Modifiers) -> Vec<Event> {
    vec![Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }]
}

pub(super) fn cursor(ctx: &egui::Context, primary: usize, secondary: usize) {
    let id = egui::Id::unique("script_text");
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap();
    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(primary),
        CCursor::new(secondary),
    )));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
}

pub(super) fn page_key(key: Key, modifiers: Modifiers) -> Vec<Event> {
    vec![Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }]
}

#[test]
fn pages_move_cursor_extend_selection_and_preserve_text() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "猫😀abc\r\n".repeat(100);
    let before = code.clone();
    frame(&ctx, &mut editor, &mut code, vec![], false);
    cursor(&ctx, 2, 2);
    assert!(frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageDown, Modifiers::NONE),
        false
    )
    .is_none());
    let next = editor.primary;
    assert!(next > 7 && next < 7 * 40, "one visible page: {next}");
    assert_eq!(next % 7, 2);
    assert_eq!(editor.secondary, next);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageDown, Modifiers::SHIFT),
        false,
    );
    assert_eq!(editor.primary, 2 * next - 2);
    assert_eq!(editor.secondary, next);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageUp, Modifiers::SHIFT),
        false,
    );
    assert_eq!((editor.primary, editor.secondary), (next, next));
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageUp, Modifiers::NONE),
        false,
    );
    assert_eq!((editor.primary, editor.secondary), (2, 2));
    assert_eq!(code, before);
}

#[test]
fn pages_respect_focus_boundaries_and_event_order() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "猫😀abc\n".repeat(100);
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 0, 0);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageUp, Modifiers::NONE),
        true,
    );
    assert_eq!(editor.primary, 0);
    cursor(&ctx, code.chars().count(), code.chars().count());
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageDown, Modifiers::NONE),
        true,
    );
    assert_eq!(editor.primary, code.chars().count());
    cursor(&ctx, 0, 0);
    let mut events = page_key(Key::PageDown, Modifiers::NONE);
    events.push(Event::Text("X".into()));
    frame(&ctx, &mut editor, &mut code, events, true);
    assert_eq!(code.chars().nth(editor.primary - 1), Some('X'));
    assert!(editor.primary > 6);
    let at = editor.primary;
    for modifiers in [Modifiers::CTRL, Modifiers::ALT, Modifiers::COMMAND] {
        frame(
            &ctx,
            &mut editor,
            &mut code,
            page_key(Key::PageDown, modifiers),
            true,
        );
        assert_eq!(editor.primary, at);
    }
    ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::unique("script_text")));
    frame(
        &ctx,
        &mut editor,
        &mut code,
        page_key(Key::PageUp, Modifiers::NONE),
        true,
    );
    let state = egui::TextEdit::load_state(&ctx, egui::Id::unique("script_text")).unwrap();
    assert_eq!(state.cursor.char_range().unwrap().primary.index.0, at);
}

#[test]
fn page_distance_tracks_viewport_height_and_scrolls_cursor_into_view() {
    let mut distances = Vec::new();
    for height in [300.0, 600.0] {
        let ctx = egui::Context::default();
        ctx.style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
        let mut editor = ScriptEditor::default();
        let mut code = "abcdefgh\n".repeat(150);
        let mut offset = 0.0;
        for step in 0..5 {
            if step == 1 {
                cursor(&ctx, 0, 0);
            }
            let events = if step == 1 || step == 2 {
                page_key(Key::PageDown, Modifiers::NONE)
            } else {
                vec![]
            };
            ctx.run_ui(
                RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, height),
                    )),
                    events,
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
        assert!(offset > 0.0, "cursor must scroll into view");
        distances.push(editor.primary);
    }
    assert!(distances[1] > distances[0], "{distances:?}");
}

#[test]
fn shortcuts_execute_selection_line_or_cell_without_editing() {
    for (modifiers, primary, secondary, expected_scope, expected_code) in [
        (Modifiers::SHIFT, 9, 9, "line", "one();"),
        (Modifiers::CTRL, 9, 9, "cell", "one();\ntwo();\n"),
        (Modifiers::SHIFT, 6, 9, "selection", "one"),
        (Modifiers::CTRL, 9, 6, "selection", "one"),
    ] {
        let ctx = egui::Context::default();
        let mut editor = ScriptEditor::default();
        let mut code = "// %%\none();\ntwo();\n// %%\nthree();".to_owned();
        let before = code.clone();
        frame(&ctx, &mut editor, &mut code, vec![], true);
        cursor(&ctx, primary, secondary);
        let request = frame(&ctx, &mut editor, &mut code, key(modifiers), true).unwrap();
        assert_eq!(request.scope, expected_scope);
        assert_eq!(request.code, expected_code);
        assert_eq!(code, before);
    }
}

#[test]
fn running_the_last_cell_opens_an_empty_cell_and_moves_there() {
    for (code, expected) in [
        (
            "// %%\none();\n// %%\ntwo();",
            "// %%\none();\n// %%\ntwo();\n// %%\n",
        ),
        ("two();\n", "two();\n// %%\n"),
    ] {
        let ctx = egui::Context::default();
        let mut editor = ScriptEditor::default();
        let mut code = code.to_owned();
        frame(&ctx, &mut editor, &mut code, vec![], true);
        let end = code.chars().count();
        cursor(&ctx, end, end);
        let request = frame(&ctx, &mut editor, &mut code, key(Modifiers::CTRL), true).unwrap();
        assert_eq!(request.code.trim_end(), "two();");
        assert_eq!(code, expected);
        let end = code.chars().count();
        assert_eq!((editor.primary, editor.secondary), (end, end));
        // The next input goes into the new cell.
        frame(
            &ctx,
            &mut editor,
            &mut code,
            vec![Event::Text("x".into())],
            true,
        );
        assert!(code.ends_with("// %%\nx"), "{code:?}");
    }
}

#[test]
fn initial_caret_is_placed_and_focused() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "a();\n// %%\n".to_owned();
    editor.caret = Some(code.chars().count());
    frame(&ctx, &mut editor, &mut code, vec![], true);
    frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![Event::Text("b".into())],
        true,
    );
    assert_eq!(code, "a();\n// %%\nb");
}

#[test]
fn enter_edits_but_shortcuts_require_focus_and_readiness() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "var x = 1;".to_owned();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 10, 10);
    assert!(frame(&ctx, &mut editor, &mut code, key(Modifiers::NONE), true).is_none());
    assert_eq!(code, "var x = 1;\n");
    for modifiers in [Modifiers::CTRL, Modifiers::SHIFT] {
        cursor(&ctx, 0, 0);
        assert!(frame(&ctx, &mut editor, &mut code, key(modifiers), false).is_none());
        assert_eq!(code, "var x = 1;\n");
    }
    ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::unique("script_text")));
    assert!(frame(&ctx, &mut editor, &mut code, key(Modifiers::CTRL), true).is_none());
}

#[test]
fn blank_line_is_not_run_and_flash_is_cleared_on_edit() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "x();\n".to_owned();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 5, 5);
    assert!(frame(&ctx, &mut editor, &mut code, key(Modifiers::SHIFT), true).is_none());
    editor.executed(execution::request(&code, 0, 0, Scope::All));
    frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![Event::Text("a".into())],
        true,
    );
    assert!(editor.flash.is_none());
}

#[test]
fn execution_indicator_is_painted_for_250_ms() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "one();\ntwo();".to_owned();
    // Execution follows an editor frame; exclude cold syntax loading from the flash timer.
    frame(&ctx, &mut editor, &mut code, vec![], true);
    for (elapsed, expected) in [(0, true), (251, false)] {
        editor.flash = Some((
            execution::request(&code, 0, 0, Scope::All),
            Instant::now() - Duration::from_millis(elapsed),
        ));
        let output = ctx.run_ui(RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                editor.show(ui, &mut code, true);
            });
        });
        let painted = output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::epaint::Shape::Rect(rect)
                if rect.fill == Color32::from_rgb(70, 220, 150))
        });
        assert_eq!(painted, expected);
        output.drop_without_applying_deltas();
    }
}

#[test]
fn snippet_insertion_is_one_undo_step_and_does_not_replace_selection() {
    let ctx = egui::Context::default();
    let mut editor = ScriptEditor::default();
    let mut code = "// %% first\nold();\n// %% next\nneighbor();".to_owned();
    let before = code.clone();
    frame(&ctx, &mut editor, &mut code, vec![], true);
    cursor(&ctx, 12, 17);
    frame(&ctx, &mut editor, &mut code, vec![], true);
    let request = editor.insert_snippet(
        &ctx,
        &mut code,
        &crate::snippets::Snippet {
            name: "new".into(),
            group: "User",
            keywords: String::new(),
            code: "var value = 1;".into(),
        },
    );
    assert_eq!(request.code, "var value = 1;");
    assert!(code.contains("old();\n// %% new\nvar value = 1;\n// %% next"));
    assert!(frame(&ctx, &mut editor, &mut code, vec![], true).is_none());
    let undo = Event::Key {
        key: Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL | Modifiers::COMMAND,
    };
    assert!(frame(&ctx, &mut editor, &mut code, vec![undo], true).is_none());
    assert_eq!(code, before);
}

#[test]
fn cell_lines_follow_marker_rows_and_clip_to_the_visible_editor() {
    let ctx = egui::Context::default();
    let mut code =
        "// %% first\r\n猫😀();\r\n  // %% next\r\n// %%%\nvar s = '// %%';\n// %% last".to_owned();
    let mut expected = Vec::new();
    let output = ctx.run_ui(RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let edit = egui::TextEdit::multiline(&mut code)
                .code_editor()
                .desired_width(500.0)
                .show(ui);
            // Simulate a viewport scrolled past the first marker, with
            // horizontal clipping as well.
            let row = |index| {
                edit.galley_pos.y + edit.galley.pos_from_cursor(CCursor::new(index)).top() + 0.5
            };
            let clip = egui::Rect::from_min_max(
                egui::pos2(edit.text_clip_rect.left() + 20.0, row(0) + 1.0),
                edit.text_clip_rect.max,
            );
            ui.set_clip_rect(clip);
            for title in ["  // %% next", "// %% last"] {
                let index = code[..code.find(title).unwrap()].chars().count();
                expected.push((row(index), clip.intersect(edit.text_clip_rect)));
            }
            paint_cell_separators(ui, &edit);
        });
    });
    let lines: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::LineSegment { points, stroke }
                if stroke.color == Color32::from_rgb(0x60, 0x60, 0x54) =>
            {
                Some((points, shape.clip_rect))
            }
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), expected.len());
    for ((points, clip), (y, expected_clip)) in lines.into_iter().zip(expected) {
        assert_eq!(clip, expected_clip);
        assert_eq!(
            *points,
            [egui::pos2(clip.left(), y), egui::pos2(clip.right(), y)]
        );
    }
    output.drop_without_applying_deltas();
}
