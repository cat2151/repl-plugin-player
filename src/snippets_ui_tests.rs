use super::*;
use egui::{epaint::Shape, Event, Pos2, RawInput, Rect};

fn text_rect(shapes: &[egui::epaint::ClippedShape], needle: &str) -> (Rect, Rect) {
    fn find(shape: &Shape, needle: &str) -> Option<Rect> {
        match shape {
            Shape::Text(text) if text.galley.text().contains(needle) => {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            }
            Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, needle)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|shape| find(&shape.shape, needle).map(|rect| (rect, shape.clip_rect)))
        .unwrap_or_else(|| panic!("missing text: {needle}"))
}

fn frame(
    ctx: &egui::Context,
    panel: &mut SnippetsPanel,
    events: Vec<Event>,
) -> (Vec<egui::epaint::ClippedShape>, Option<Snippet>) {
    let mut chosen = None;
    let output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 500.0))),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                chosen = panel.browser(ui, true);
            });
        },
    );
    let shapes = output.shapes.clone();
    output.drop_without_applying_deltas();
    (shapes, chosen)
}

#[test]
fn english_timeline_name_is_searchable_and_selectable() {
    let ctx = egui::Context::default();
    let mut panel = SnippetsPanel {
        entries: vec![Snippet {
            name: "play".into(),
            group: "Built-in",
            keywords: String::new(),
            code: String::new(),
        }],
        filter: "play time".into(),
        ..Default::default()
    };
    assert!(SnippetsPanel::matches_filter(
        &panel.entries[0],
        &panel.filter
    ));
    let output = ctx.run_ui(
        egui::RawInput {
            events: vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            ..Default::default()
        },
        |ui| {
            panel.keyboard(ui.ctx(), true);
        },
    );
    output.drop_without_applying_deltas();
    assert_eq!(panel.preview, Some(0));
    assert!(!SnippetsPanel::matches_filter(
        &panel.entries[0],
        "play unrelated"
    ));
}

#[test]
fn timeline_loop_is_searchable_and_rendered_by_display_name() {
    let ctx = egui::Context::default();
    let mut panel = SnippetsPanel {
        entries: Library::default().load().unwrap(),
        filter: "Loop timeline".into(),
        ..Default::default()
    };
    let (shapes, _) = frame(&ctx, &mut panel, vec![]);
    text_rect(&shapes, "Loop timeline");
    assert!(panel.entries.iter().any(|snippet| {
        snippet.name == "play-loop" && SnippetsPanel::matches_filter(snippet, "loop timeline")
    }));
}

#[test]
fn long_preview_stays_beside_list_and_survives_pointer_transfer() {
    let ctx = egui::Context::default();
    let mut panel = SnippetsPanel {
        entries: vec![
            Snippet {
                name: "short-example".into(),
                group: "User",
                keywords: String::new(),
                code: "short();".into(),
            },
            Snippet {
                name: "long-example".into(),
                group: "User",
                keywords: String::new(),
                code: "long();\n".repeat(200),
            },
        ],
        ..Default::default()
    };
    let (shapes, _) = frame(&ctx, &mut panel, vec![]);
    let before = text_rect(&shapes, "long-example").0;
    let point = before.center();
    let (shapes, chosen) = frame(&ctx, &mut panel, vec![Event::PointerMoved(point)]);
    assert!(chosen.is_none(), "hover must never execute a snippet");
    assert_eq!(panel.preview, Some(1));
    assert_eq!(text_rect(&shapes, "long-example   |").0, before);
    let (code, clip) = text_rect(&shapes, "long();");
    assert!(
        clip.left() > before.right(),
        "preview must not cover the row"
    );
    assert!(
        code.height() > clip.height(),
        "long code must be clipped to its scroll area"
    );
    assert!(clip.height() > 450.0 && clip.height() <= 500.0);
    let (shapes, _) = frame(&ctx, &mut panel, vec![Event::PointerMoved(clip.center())]);
    assert_eq!(panel.preview, Some(1));
    assert_eq!(text_rect(&shapes, "long-example   |").0, before);
    text_rect(&shapes, "long();");
    frame(
        &ctx,
        &mut panel,
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -160.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let (shapes, _) = frame(&ctx, &mut panel, vec![]);
    assert!(text_rect(&shapes, "long();").0.top() < code.top());
    assert_eq!(text_rect(&shapes, "long-example   |").0, before);
    assert_eq!(panel.preview, Some(1));

    // A normal row click still inserts/runs the same snippet.
    frame(
        &ctx,
        &mut panel,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let (_, chosen) = frame(
        &ctx,
        &mut panel,
        vec![Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(chosen.unwrap().name, "long-example");

    panel.filter = "short-example".into();
    frame(&ctx, &mut panel, vec![Event::PointerGone]);
    assert_eq!(
        panel.preview,
        Some(0),
        "filtering selects the first matching row"
    );
}

#[test]
fn window_and_preview_follow_viewport_height() {
    let ctx = egui::Context::default();
    let mut panel = SnippetsPanel {
        open: true,
        preview: Some(0),
        entries: vec![Snippet {
            name: "viewport-example".into(),
            group: "User",
            keywords: String::new(),
            code: "viewport_example();\n".repeat(200),
        }],
        ..Default::default()
    };
    let mut previous_preview_height = 0.0;
    for height in [500.0, 900.0, 400.0] {
        // Allow the window and bottom-aligned footer to settle after resizing.
        let mut shapes = Vec::new();
        for _ in 0..3 {
            let output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, height))),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        assert!(panel.show(ui, "", 0, true).is_none());
                    });
                },
            );
            shapes = output.shapes.clone();
            output.drop_without_applying_deltas();
        }
        let window = egui::AreaState::load(&ctx, egui::Id::unique("snippet_window"))
            .unwrap()
            .rect();
        assert!(window.top().abs() <= 1.0, "{window:?}");
        assert!((window.bottom() - height).abs() <= 1.0, "{window:?}");
        let (_, preview) = text_rect(&shapes, "viewport_example();");
        assert!(preview.height() > height - 200.0, "{preview:?}");
        let (save, clip) = text_rect(&shapes, crate::i18n::t("m029"));
        assert!(clip.contains_rect(save), "save button must remain visible");
        assert!(save.top() >= preview.bottom());
        if height == 900.0 {
            assert!(preview.height() > previous_preview_height + 350.0);
        }
        previous_preview_height = preview.height();
    }
}

fn editor_frame(
    ctx: &egui::Context,
    editor: &mut crate::script_editor::ScriptEditor,
    code: &mut String,
    events: Vec<Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 500.0))),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                assert!(editor.show(ui, code, true).is_none());
            });
        },
    );
    let shapes = output.shapes.clone();
    output.drop_without_applying_deltas();
    shapes
}

fn click(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Opens the snippets window with a real button click, as the user does.
fn open_snippets(ctx: &egui::Context) -> (crate::script_editor::ScriptEditor, String) {
    let mut editor = crate::script_editor::ScriptEditor::default();
    let mut code = "let a = 1;
"
    .to_string();
    let shapes = editor_frame(ctx, &mut editor, &mut code, vec![]);
    let button = text_rect(&shapes, crate::i18n::t("m022")).0.center();
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("script_text")));
    editor_frame(ctx, &mut editor, &mut code, vec![]);
    editor_frame(
        ctx,
        &mut editor,
        &mut code,
        vec![Event::PointerMoved(button), click(button, true)],
    );
    editor_frame(ctx, &mut editor, &mut code, vec![click(button, false)]);
    editor_frame(ctx, &mut editor, &mut code, vec![]);
    assert!(editor.snippets().open, "snippets window must be open");
    assert!(
        ctx.memory(|memory| memory.has_focus(egui::Id::unique("snippet_filter"))),
        "opening must focus the filter"
    );
    (editor, code)
}

fn assert_slash_focuses_filter(
    ctx: &egui::Context,
    editor: &mut crate::script_editor::ScriptEditor,
    code: &mut String,
) {
    let original = code.clone();
    editor_frame(ctx, editor, code, slash());
    editor_frame(ctx, editor, code, vec![]);
    let focused = ctx.memory(|memory| memory.focused());
    let filter = egui::Id::unique("snippet_filter");
    assert!(
        focused == Some(filter) && *code == original && editor.snippets().filter.is_empty(),
        "focus={focused:?} (want {filter:?}) code={code:?} filter={:?}",
        editor.snippets().filter
    );
}

#[test]
fn slash_moves_focus_from_code_to_open_snippets_filter() {
    let ctx = egui::Context::default();
    let (mut editor, mut code) = open_snippets(&ctx);
    let script = egui::Id::unique("script_text");
    ctx.memory_mut(|memory| memory.request_focus(script));
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    assert!(ctx.memory(|memory| memory.has_focus(script)));
    assert_slash_focuses_filter(&ctx, &mut editor, &mut code);
}

#[test]
fn slash_is_text_inside_snippets_filter() {
    let ctx = egui::Context::default();
    let (mut editor, mut code) = open_snippets(&ctx);
    let original = code.clone();
    editor_frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![Event::Text("a/b".into())],
    );
    editor_frame(&ctx, &mut editor, &mut code, slash());
    assert_eq!(editor.snippets().filter, "a/b/");
    assert_eq!(code, original);
}

#[test]
fn slash_is_text_inside_snippet_name() {
    let ctx = egui::Context::default();
    let (mut editor, mut code) = open_snippets(&ctx);
    let name = egui::Id::unique("snippet_name");
    ctx.memory_mut(|memory| memory.request_focus(name));
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    assert!(ctx.memory(|memory| memory.has_focus(name)));
    let original = code.clone();
    editor_frame(&ctx, &mut editor, &mut code, slash());
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    assert_eq!(editor.snippets().name, "/");
    assert!(editor.snippets().filter.is_empty());
    assert_eq!(code, original);
    assert!(ctx.memory(|memory| memory.has_focus(name)));
}

#[path = "snippets_ui_escape_tests.rs"]
mod escape;

#[path = "snippets_ui_tab_tests.rs"]
mod tab;

/// What the window receives when `/` is typed: the key, then its text.
fn slash() -> Vec<Event> {
    vec![
        key(egui::Key::Slash, egui::Modifiers::NONE),
        Event::Text("/".into()),
    ]
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn ctrl_shift_p_opens_snippets_with_filter_focused() {
    let ctx = egui::Context::default();
    let mut editor = crate::script_editor::ScriptEditor::default();
    let mut code = "let a = 1;\n".to_string();
    let script = egui::Id::unique("script_text");
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(script));
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    let shortcut = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
    editor_frame(
        &ctx,
        &mut editor,
        &mut code,
        vec![key(egui::Key::P, shortcut)],
    );
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    assert!(editor.snippets().open);
    assert_eq!(code, "let a = 1;\n");
    assert!(ctx.memory(|memory| memory.has_focus(egui::Id::unique("snippet_filter"))));
    // Typing goes straight into the filter.
    editor_frame(&ctx, &mut editor, &mut code, vec![Event::Text("ab".into())]);
    assert_eq!(editor.snippets().filter, "ab");
}

#[path = "snippets_ui_selection_tests.rs"]
mod selection;

#[test]
fn slash_goes_to_code_while_snippets_closed() {
    let ctx = egui::Context::default();
    let mut editor = crate::script_editor::ScriptEditor::default();
    let mut code = String::new();
    let script = egui::Id::unique("script_text");
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(script));
    editor_frame(&ctx, &mut editor, &mut code, vec![]);
    editor_frame(&ctx, &mut editor, &mut code, slash());
    assert!(!editor.snippets().open);
    assert_eq!(code, "/");
}
