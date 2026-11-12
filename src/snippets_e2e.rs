//! Inserted user snippets run exactly once and alone; the snippets and the
//! editor text are cells of `tests/e2e/snippets.js`.
use crate::{
    control::Control,
    e2e_script::Script,
    script_editor::ScriptEditor,
    snippets::{Library, Snippet},
};

fn snippet(script: &Script, title: &str) -> Result<Snippet, String> {
    Ok(Snippet {
        name: title.into(),
        group: "User",
        code: script.cell(title)?.into(),
    })
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    let library = Library::default();
    let entries = library.load()?;
    let mut editor = ScriptEditor::default();
    let initial = editor.initial_code();
    if !entries
        .iter()
        .any(|s| s.group == "Built-in" && s.code == initial)
    {
        return Err("Startup code is not a managed built-in snippet".into());
    }
    let add = snippet(script, "add clip")?;
    let ctx = eframe::egui::Context::default();
    let (from, to) = (script.marker("editor text")?, script.marker("add clip")?);
    let mut document: String = script.text.chars().skip(from).take(to - from).collect();
    for _ in 0..2 {
        let request = editor.insert_snippet(&ctx, &mut document, &add);
        app.run_request(&request)?;
    }
    let snapshot = app.read_timeline()?;
    if snapshot["tracks"][0]["clips"].as_array().map(Vec::len) != Some(2)
        || app.log.execution != 2
        || app.observe("snippets do not implicitly start audio")?["engineEnabled"] != false
    {
        return Err("Snippet click must run only its code, exactly once".into());
    }
    let failing = snippet(script, "failing snippet")?;
    let request = editor.insert_snippet(&ctx, &mut document, &failing);
    let effect = script.cell("effect")?;
    if app.run_request(&request).is_ok()
        || !document.contains(&failing.code)
        || app.eval(effect, "snippet error retains effects")? != "42"
    {
        return Err("Snippet failure must retain code and prior effects".into());
    }
    let recovery = snippet(script, "recovery")?;
    app.run_request(&editor.insert_snippet(&ctx, &mut document, &recovery))?;
    if app.eval(effect, "snippet recovery")? != "43" {
        return Err("Could not recover after snippet failure".into());
    }
    Ok(())
}
