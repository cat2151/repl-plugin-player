//! Real runtime coverage for the same range requests used by the GUI.
use crate::{
    control::Control,
    execution::{request, Scope},
};

pub fn run(app: &mut Control) -> Result<(), String> {
    let code = "throw new Error('outside cell');\n// %% setup\nvar partialValue = 40;\n// %% edit\npartialValue += 2;\n// %% never\nthrow new Error('neighbor');";
    for needle in ["var partialValue", "partialValue +="] {
        let cursor = code[..code.find(needle).unwrap()].chars().count();
        app.run_request(&request(code, cursor, cursor, Scope::Cell))?;
    }
    let selection = "throw new Error('before'); partialValue += 1; throw new Error('after');";
    let start = selection.find("partialValue").unwrap();
    let end = start + "partialValue += 1;".len();
    app.run_request(&request(selection, end, start, Scope::Cell))?;
    if app.eval("partialValue", "verify partial runs retain globals")? != "43" {
        return Err("Partial runs did not share their REPL globals".into());
    }
    let failing = "// %% error\npartialValue = 44; throw new Error('expected fragment error');\n// %% never\npartialValue = -1;";
    if app
        .run_request(&request(failing, 0, 0, Scope::Cell))
        .is_ok()
    {
        return Err("Cell runtime error unexpectedly succeeded".into());
    }
    app.run_request(&request(
        "partialValue += 1;\nthrow new Error('next line');",
        0,
        0,
        Scope::Line,
    ))?;
    if app.eval("partialValue", "verify recovery without rollback")? != "45" {
        return Err("Partial run error lost effects or prevented recovery".into());
    }
    let sample = crate::e2e::TIMELINE;
    app.run_request(&request(sample, 0, 0, Scope::Cell))?;
    let cursor = sample[..sample
        .match_indices("// %%")
        .nth(1)
        .ok_or("No second sample cell")?
        .0]
        .chars()
        .count();
    for _ in 0..2 {
        app.run_request(&request(sample, cursor, cursor, Scope::Cell))?;
    }
    let snapshot = app.read_timeline()?;
    if snapshot["tracks"][0]["clips"].as_array().map(Vec::len) != Some(2)
        || snapshot["state"]["isPlaying"] != false
        || app.observe("partial model edits do not enable engine")?["engineEnabled"] != false
    {
        return Err("Partial sample cells did not retain two clips with audio stopped".into());
    }
    Ok(())
}
