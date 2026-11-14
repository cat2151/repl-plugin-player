use super::*;

fn show_frame(
    ctx: &egui::Context,
    panel: &mut SnippetsPanel,
    events: Vec<Event>,
    ready: bool,
) -> Option<Snippet> {
    let mut chosen = None;
    ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 500.0))),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                chosen = panel.show(ui, "", 0, ready);
            });
        },
    )
    .drop_without_applying_deltas();
    chosen
}

#[test]
fn enter_runs_first_result_on_open_and_after_typing_filter() {
    for query in ["", "keep", "missing", "["] {
        let ctx = egui::Context::default();
        let mut panel = SnippetsPanel {
            open: true,
            focus_filter: true,
            entries: ["other", "keep-a", "keep-b"]
                .into_iter()
                .map(|name| Snippet {
                    name: name.into(),
                    group: "User",
                    code: "42".into(),
                })
                .collect(),
            ..Default::default()
        };
        for _ in 0..3 {
            show_frame(&ctx, &mut panel, vec![], true);
        }
        assert_eq!(panel.preview, Some(0));
        // Even a still-matching selection must return to the first result on edits.
        panel.preview = Some(2);
        if !query.is_empty() {
            show_frame(&ctx, &mut panel, vec![Event::Text(query.into())], true);
        } else {
            panel.preview = Some(0);
        }
        let expected = match query {
            "" => Some(0),
            "keep" => Some(1),
            _ => None,
        };
        assert_eq!(panel.preview, expected, "{query}");
        // Readiness still gates execution without losing the default selection.
        assert!(show_frame(
            &ctx,
            &mut panel,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            false,
        )
        .is_none());
        let chosen = show_frame(
            &ctx,
            &mut panel,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            true,
        );
        assert_eq!(
            chosen.as_ref().map(|snippet| snippet.name.as_str()),
            expected.map(|index| panel.entries[index].name.as_str()),
            "{query}",
        );
    }
}

#[test]
fn arrows_in_filter_select_matching_rows_and_enter_runs() {
    let ctx = egui::Context::default();
    let snippet = |name: &str| Snippet {
        name: name.into(),
        group: "User",
        code: format!("{name}();"),
    };
    let mut panel = SnippetsPanel {
        open: true,
        filter: "keep".into(),
        entries: vec![snippet("keep-a"), snippet("drop"), snippet("keep-b")],
        ..Default::default()
    };
    let run = |panel: &mut SnippetsPanel, events: Vec<Event>| {
        let mut chosen = None;
        let output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 500.0))),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    chosen = panel.show(ui, "", 0, true);
                });
            },
        );
        output.drop_without_applying_deltas();
        chosen
    };
    run(&mut panel, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("snippet_filter")));
    run(&mut panel, vec![]);
    let none = egui::Modifiers::NONE;
    assert_eq!(
        panel.preview,
        Some(0),
        "the first match is selected by default"
    );
    run(&mut panel, vec![key(egui::Key::ArrowDown, none)]);
    assert_eq!(panel.preview, Some(2), "filtered-out rows are skipped");
    run(&mut panel, vec![key(egui::Key::ArrowDown, none)]);
    assert_eq!(panel.preview, Some(2), "selection stops at the last row");
    run(&mut panel, vec![key(egui::Key::ArrowUp, none)]);
    assert_eq!(panel.preview, Some(0));
    assert_eq!(panel.filter, "keep", "arrows must not edit the filter");
    let chosen = run(&mut panel, vec![key(egui::Key::Enter, none)]);
    assert_eq!(chosen.unwrap().name, "keep-a");
    assert!(!panel.open);
}
