//! Real runtime coverage for the same range requests used by the GUI, over the
//! editor text `tests/e2e/partial-run.js`.
use crate::{
    control::Control,
    e2e_script::Script,
    execution::{request, Scope},
};

/// The character offset of line `line` (0 is the marker line) of the cell `title`.
fn line_start(script: &Script, title: &str, line: usize) -> Result<usize, String> {
    let marker = script.marker(title)?;
    let lines = script.text.chars().skip(marker).collect::<String>();
    let skipped: usize = lines
        .split_inclusive('\n')
        .take(line)
        .map(|l| l.chars().count())
        .sum();
    Ok(marker + skipped)
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    let text = script.text.as_str();
    let at = |cursor: usize, scope: Scope| request(text, cursor, cursor, scope);
    for title in ["setup", "edit"] {
        app.run_request(&at(line_start(script, title, 1)?, Scope::Cell))?;
    }
    let start = line_start(script, "selection", 2)?;
    let end = line_start(script, "selection", 3)? - 1;
    app.run_request(&request(text, end, start, Scope::Cell))?;
    let value = script.cell("value")?;
    if app.eval(value, "verify partial runs retain globals")? != "43" {
        return Err("Partial runs did not share their REPL globals".into());
    }
    if app
        .run_request(&at(script.marker("failing")?, Scope::Cell))
        .is_ok()
    {
        return Err("Cell runtime error unexpectedly succeeded".into());
    }
    app.run_request(&at(line_start(script, "line", 1)?, Scope::Line))?;
    if app.eval(value, "verify recovery without rollback")? != "45" {
        return Err("Partial run error lost effects or prevented recovery".into());
    }
    app.run_request(&at(line_start(script, "track and reset", 1)?, Scope::Cell))?;
    let note = script.marker("one note: pitch 60 from beat 1 to beat 3")?;
    for _ in 0..2 {
        app.run_request(&at(note, Scope::Cell))?;
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
