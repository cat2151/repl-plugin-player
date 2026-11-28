//! `--e2e keys`: pressing the key `[scripts]` gives `builtin/play-loop.js` starts the
//! loop, with the focus in the editor and outside it. The app's `init.js` binds the
//! key; the test-owned cells are `tests/e2e/keys.js`.
use super::harness::{Harness, DEFAULT};
use crate::{control::Control, e2e_script::Script};
use eframe::egui;

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

fn control(harness: &mut Harness) -> Result<&mut Control, String> {
    harness.editor.control.as_mut().ok_or("no engine".into())
}

fn playing(harness: &mut Harness, script: &Script) -> Result<bool, String> {
    let playing = control(harness)?.eval(script.cell("playing")?, "E2E: keys playing")?;
    Ok(playing == "true")
}

fn check(harness: &mut Harness, script: &Script) -> Result<(), String> {
    // The engine is ready, as `App::ui` finds it after startup.
    harness.editor.startup.update(true, false);
    harness.settle(DEFAULT);
    control(harness)?.run(script.cell("clip")?)?;
    let key = control(harness)?.eval(script.cell("key")?, "E2E: keys key")?;
    let key: String = serde_json::from_str(&key).map_err(|error| format!("{key}: {error}"))?;
    if key.is_empty() {
        return Err("keybinds.toml binds no key to builtin/play-loop.js".into());
    }
    let press =
        |pressed| crate::keybinds::key_event(&key, pressed).ok_or(format!("{key} is not a key"));
    let editor = egui::Id::unique("script_text");
    let mut results = vec![];
    for focused in [true, false] {
        control(harness)?.run(script.cell("stop")?)?;
        if playing(harness, script)? {
            return Err("stopPlayback left the loop playing".into());
        }
        harness.ctx.memory_mut(|memory| match focused {
            true => memory.request_focus(editor),
            false => memory.surrender_focus(editor),
        });
        harness.settle(DEFAULT);
        let has_focus = harness.ctx.memory(|memory| memory.has_focus(editor));
        if has_focus != focused {
            return Err(format!("the editor's focus is {has_focus}, not {focused}"));
        }
        harness.frame(DEFAULT, vec![press(true)?]);
        harness.frame(DEFAULT, vec![press(false)?]);
        harness.settle(DEFAULT);
        if !playing(harness, script)? {
            return Err(format!(
                "{key} did not start the loop (editor focus: {focused})"
            ));
        }
        results.push(serde_json::json!({"editorFocus": focused, "playing": true}));
    }
    control(harness)?.run(script.cell("stop")?)?;
    control(harness)?.log.event(
        "keys_acceptance",
        serde_json::json!({"key": key, "runs": results}),
    )
}
