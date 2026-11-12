use super::*;

fn frame(ctx: &egui::Context, pending: &mut Pending) {
    ctx.run_ui(egui::RawInput::default(), |_| {
        pending.show_overlay(ctx);
        // Even a visible layout pass must finish before execution can begin.
        assert!(pending.take_ready(ctx).is_none());
    })
    .drop_without_applying_deltas();
}

#[test]
fn presents_before_evaluation_and_clears_for_silent_or_failed_scripts() {
    let ctx = egui::Context::default();
    let mut pending = Pending::default();
    for code in ["var silent = 1;", "throw new Error('failed');"] {
        pending.queue(crate::execution::request(
            code,
            0,
            0,
            crate::execution::Scope::All,
        ));
        assert!(pending.take_ready(&ctx).is_none());
        for _ in 0..3 {
            frame(&ctx, &mut pending);
            if pending.presented_frame.is_some() {
                break;
            }
        }
        assert!(ctx.memory(|m| m.top_modal_layer().is_some()));
        assert_eq!(pending.take_ready(&ctx).unwrap().code, code);
        // No audio event or successful evaluation is needed to release the UI.
        assert!(!pending.is_pending());
        assert!(pending.take_ready(&ctx).is_none());
        frame(&ctx, &mut pending);
        frame(&ctx, &mut pending);
        assert!(ctx.memory(|m| m.top_modal_layer().is_none()));
    }
}

#[test]
fn presents_the_first_evaluation_of_any_scope_and_every_snippet() {
    let ctx = egui::Context::default();
    let mut pending = Pending::default();
    let cell = crate::execution::request("1", 0, 0, crate::execution::Scope::Cell);
    let mut snippet = cell.clone();
    snippet.scope = "snippet";
    assert!(pending.presents(&cell));
    pending.queue(cell.clone());
    for _ in 0..3 {
        frame(&ctx, &mut pending);
        if pending.presented_frame.is_some() {
            break;
        }
    }
    assert!(pending.take_ready(&ctx).is_some());
    assert!(!pending.presents(&cell));
    assert!(pending.presents(&snippet));
}
