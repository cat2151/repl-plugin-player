use super::*;

fn frame(ctx: &egui::Context, startup: &mut Startup, code: &mut String, events: Vec<egui::Event>) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1250.0, 800.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            egui::Panel::left("code").show(ui, |ui| {
                if startup.is_preparing() {
                    ui.disable();
                }
                startup.focus_editor(ui);
                ui.add(egui::TextEdit::multiline(code).id(egui::Id::unique("script_text")));
            });
            egui::CentralPanel::default().show(ui, |ui| {
                if startup.is_preparing() {
                    ui.disable();
                }
                ui.text_edit_singleline(&mut String::new());
            });
            let can_initialize = startup.can_initialize(ctx);
            startup.show_overlay(ctx);
            // Layout passes must not start native initialization before rendering.
            assert_eq!(startup.can_initialize(ctx), can_initialize);
        },
    )
    .drop_without_applying_deltas();
}

#[test]
fn startup_blocks_editing_then_focuses_editor_once() {
    let ctx = egui::Context::default();
    let mut startup = Startup::default();
    let mut code = "initial".to_owned();
    assert!(!startup.can_initialize(&ctx));
    frame(&ctx, &mut startup, &mut code, vec![]);
    assert!(!startup.can_initialize(&ctx)); // Invisible modal sizing frame.
    frame(&ctx, &mut startup, &mut code, vec![]);
    assert!(startup.can_initialize(&ctx));
    assert!(ctx.memory(|m| m.top_modal_layer().is_some()));
    ctx.memory_mut(|m| m.request_focus(egui::Id::unique("script_text")));
    frame(
        &ctx,
        &mut startup,
        &mut code,
        vec![egui::Event::Text("blocked".into())],
    );
    assert_eq!(code, "initial");

    startup.update(true, false);
    // Allow the previous frame's modal layer to clear.
    frame(&ctx, &mut startup, &mut code, vec![]);
    frame(&ctx, &mut startup, &mut code, vec![]);
    assert!(ctx.memory(|m| m.top_modal_layer().is_none()));
    assert!(ctx.memory(|m| m.has_focus(egui::Id::unique("script_text"))));
    frame(
        &ctx,
        &mut startup,
        &mut code,
        vec![egui::Event::Text("typed".into())],
    );
    assert!(code.contains("typed"));

    ctx.memory_mut(|m| m.surrender_focus(egui::Id::unique("script_text")));
    startup.update(false, false); // Ordinary Stop must not restart the overlay or steal focus.
    frame(&ctx, &mut startup, &mut code, vec![]);
    assert!(!startup.is_preparing());
    assert!(!ctx.memory(|m| m.has_focus(egui::Id::unique("script_text"))))
}

#[test]
fn initialization_failure_removes_overlay_without_focusing_editor() {
    let ctx = egui::Context::default();
    let mut startup = Startup::default();
    let mut code = String::new();
    frame(&ctx, &mut startup, &mut code, vec![]);
    startup.update(false, true);
    frame(&ctx, &mut startup, &mut code, vec![]);
    frame(&ctx, &mut startup, &mut code, vec![]);
    assert!(!startup.is_preparing());
    assert!(ctx.memory(|m| m.top_modal_layer().is_none()));
    assert!(!startup.focused);
}
