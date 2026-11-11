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
fn long_preview_stays_beside_list_and_survives_pointer_transfer() {
    let ctx = egui::Context::default();
    let mut panel = SnippetsPanel {
        entries: vec![
            Snippet {
                name: "short-example".into(),
                group: "User",
                code: "short();".into(),
            },
            Snippet {
                name: "long-example".into(),
                group: "User",
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
        panel.preview, None,
        "filtered-out rows must not retain a preview"
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
