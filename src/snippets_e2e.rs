use crate::{
    control::Control,
    script_editor::ScriptEditor,
    snippets::{Library, Snippet},
};

pub fn run(app: &mut Control) -> Result<(), String> {
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
    let add = &Snippet {
        name: "add-clip".into(),
        group: "User",
        code: "uapmd.timeline.createEmptyMidiClip(include('builtin/lib/plugins.js').trackFor(globalThis.surgeId).trackIndex, 0, 480, 120);".into(),
    };
    let ctx = eframe::egui::Context::default();
    let mut document = "throw new Error('old cell must not run');\n// %% next\nthrow new Error('neighbor must not run');".to_owned();
    for _ in 0..2 {
        let request = editor.insert_snippet(&ctx, &mut document, add);
        app.run_request(&request)?;
    }
    let snapshot = app.read_timeline()?;
    if snapshot["tracks"][0]["clips"].as_array().map(Vec::len) != Some(2)
        || app.log.execution != 2
        || app.observe("snippets do not implicitly start audio")?["engineEnabled"] != false
    {
        return Err("Snippet click must run only its code, exactly once".into());
    }
    let failing = Snippet {
        name: "error".into(),
        group: "User",
        code: "globalThis.snippetEffect = 42; throw new Error('snippet error');".into(),
    };
    let request = editor.insert_snippet(&ctx, &mut document, &failing);
    if app.run_request(&request).is_ok()
        || !document.contains(&failing.code)
        || app.eval("snippetEffect", "snippet error retains effects")? != "42"
    {
        return Err("Snippet failure must retain code and prior effects".into());
    }
    let recovery = Snippet {
        name: "recovery".into(),
        group: "User",
        code: "snippetEffect += 1;".into(),
    };
    app.run_request(&editor.insert_snippet(&ctx, &mut document, &recovery))?;
    if app.eval("snippetEffect", "snippet recovery")? != "43" {
        return Err("Could not recover after snippet failure".into());
    }
    Ok(())
}
