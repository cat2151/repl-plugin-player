//! `--e2e dock-script-ui`: a `repl.ui` window starts in its own window, goes into the
//! tree by its button, moves the real plugin parameter by its slider as a tab, goes
//! back out by the tab menu, and is gone after `repl.ui.close` and after its tab's ×.
//! The test-owned cells are `tests/e2e/dock-script-ui.js`.
use super::{
    harness::{Harness, DEFAULT},
    Pane,
};
use crate::{
    control::Control,
    e2e_script::Script,
    script_ui::{Binding, Parameters},
};
use eframe::egui::{self, PointerButton};

const ID: &str = "e2e-dock";

/// Borrows the ready `app` as the editor's engine and hands it back.
pub(crate) fn run(app: Control, script: &Script) -> (Control, Result<(), String>) {
    crate::script_ui::enable(true);
    let mut harness = Harness::new();
    harness.editor.control = Some(app);
    let result = check(&mut harness, script);
    crate::script_ui::enable(false);
    let app = harness
        .editor
        .control
        .take()
        .expect("the engine stays in the editor");
    (app, result)
}

fn control(harness: &mut Harness) -> Result<&mut Control, String> {
    harness.editor.control.as_mut().ok_or("no engine".into())
}

/// Where the script pane is: in the tree, in its own window, both or neither.
fn place(harness: &Harness) -> (bool, bool) {
    let pane = Pane::Script(ID.into());
    let floating = harness.editor.layout.floating_panes().contains(&pane);
    (harness.in_tree(&pane), floating)
}

fn open(harness: &mut Harness, script: &Script) -> Result<(), String> {
    control(harness)?.run(script.cell("open")?)?;
    harness.settle(DEFAULT);
    if place(harness) != (false, true) {
        return Err(format!(
            "a new window is not in its own window: {:?}",
            place(harness)
        ));
    }
    Ok(())
}

fn check(harness: &mut Harness, script: &Script) -> Result<(), String> {
    harness.settle(DEFAULT);
    open(harness, script)?;
    let window = egui::Id::unique(Pane::Script(ID.into()).viewport());
    let area = harness.ctx.memory(|memory| memory.area_rect(window));
    if !area.is_some_and(|rect| rect.width() > 0.0 && rect.height() > 0.0) {
        return Err(format!(
            "the script window's own window is not drawn: {area:?}"
        ));
    }
    harness.dock()?;
    if place(harness) != (true, false) {
        return Err(format!(
            "its button did not dock the window: {:?}",
            place(harness)
        ));
    }
    let (binding, original) = slider(harness)?;
    let result = drag(harness, script, &binding, original);
    let restored = control(harness)?.set(&binding, original);
    result.and(restored)
}

/// The slider's binding and the parameter's value before the drag.
fn slider(harness: &mut Harness) -> Result<(Binding, f64), String> {
    let window = crate::script_ui::snapshot()
        .into_iter()
        .find(|window| window.id == ID)
        .ok_or("the open cell registered no window")?;
    let binding = window.widgets[0].binding.clone().ok_or("no binding")?;
    let value = control(harness)?.parameter(&binding)?.value;
    Ok((binding, value))
}

fn drag(
    harness: &mut Harness,
    script: &Script,
    binding: &Binding,
    original: f64,
) -> Result<(), String> {
    let label = crate::script_ui::snapshot()[0].widgets[0].label.clone();
    let label = harness
        .text(&label)
        .ok_or("the docked tab does not draw its slider")?;
    let tiles = &harness.editor.layout.tree.tiles;
    let pane = tiles
        .find_pane(&Pane::Script(ID.into()))
        .and_then(|tile| tiles.rect(tile))
        .ok_or("the script tab has no rect")?;
    // The slider's rail starts at the pane's left, inside the panel frame's margin.
    harness.click(
        egui::pos2(pane.left() + 8.0 + 65.0, label.center().y),
        PointerButton::Primary,
    );
    let changed = control(harness)?.parameter(binding)?.value;
    if (changed - original).abs() < 1e-8 {
        return Err("the tab's slider did not change the plugin parameter".into());
    }
    harness.float(&Pane::Script(ID.into()))?;
    if place(harness) != (false, true) {
        return Err(format!(
            "the tab menu did not float the window: {:?}",
            place(harness)
        ));
    }
    control(harness)?.run(script.cell("close")?)?;
    harness.settle(DEFAULT);
    if place(harness) != (false, false) || harness.text(crate::i18n::t("pane_dock")).is_some() {
        return Err("repl.ui.close left the window's own window".into());
    }
    open(harness, script)?;
    harness.dock()?;
    let tab = harness.text("E2E dock").ok_or("no tab of the window")?;
    // The tab's × sits right of its title.
    harness.click(
        egui::pos2(tab.right() + 10.0, tab.center().y),
        PointerButton::Primary,
    );
    if place(harness) != (false, false) || !crate::script_ui::snapshot().is_empty() {
        return Err(format!(
            "the tab's × did not close the window: {:?}",
            place(harness)
        ));
    }
    control(harness)?.log.event(
        "dock_script_ui_acceptance",
        serde_json::json!({"original": original, "pointerValue": changed}),
    )
}
