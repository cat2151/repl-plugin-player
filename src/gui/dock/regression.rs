//! `--e2e dock`: the editor pane, moved into its own window by its tab menu, runs the
//! editor against the real engine, and its button puts it back into the tree.
//! The test-owned cells are `tests/e2e/dock.js`.
use super::{
    harness::{Harness, DEFAULT},
    Pane,
};
use crate::{control::Control, e2e_script::Script};
use eframe::egui::PointerButton;

/// Borrows the ready `app` as the editor's engine and hands it back.
pub(crate) fn run(app: Control, script: &Script) -> (Control, Result<(), String>) {
    let mut harness = Harness::new();
    harness.editor.control = Some(app);
    let result = check(&mut harness, script);
    let app = harness
        .editor
        .control
        .take()
        .expect("the engine stays in the editor");
    (app, result)
}

fn check(harness: &mut Harness, script: &Script) -> Result<(), String> {
    harness.editor.code = script.cell("run")?.to_owned();
    harness.settle(DEFAULT);
    harness.float(&Pane::Editor)?;
    if harness.in_tree(&Pane::Editor) || harness.editor.layout.floating_panes() != [Pane::Editor] {
        return Err("the tab menu did not move the editor into its own window".into());
    }
    let run = harness
        .text(crate::i18n::t("m012"))
        .ok_or("the floating editor has no Run all button")?;
    harness.click(run.center(), PointerButton::Primary);
    let control = harness.editor.control.as_mut().ok_or("no engine")?;
    let runs = control.eval(script.cell("runs")?, "E2E: dock runs")?;
    if runs.parse::<f64>() != Ok(1.0) {
        return Err(format!("the floating editor ran the editor {runs} times"));
    }
    harness.dock()?;
    if !harness.in_tree(&Pane::Editor) || !harness.editor.layout.floating.is_empty() {
        return Err("the window's button did not put the editor back".into());
    }
    let control = harness.editor.control.as_mut().ok_or("no engine")?;
    control
        .log
        .event("dock_acceptance", serde_json::json!({"runs": runs}))
}
