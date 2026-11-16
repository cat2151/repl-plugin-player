#[path = "../patches/uapmd/script-api/state_regression.rs"]
mod state_load;

use crate::{control::Control, e2e_script::Script, log};
use serde_json::json;
use std::time::{Duration, Instant};

/// The instance id of the instrument on track 0, from the case's `instrument` cell.
pub(crate) fn instrument(app: &mut Control, script: &Script) -> Result<i64, String> {
    let id = app.eval(script.cell("instrument")?, "E2E: instrument on track 0")?;
    id.parse::<f64>()
        .ok()
        .filter(|id| *id >= 0.0 && id.fract() == 0.0)
        .map(|id| id as i64)
        .ok_or_else(|| format!("No instrument on track 0: {id}"))
}

/// Cases whose checks are Rust; their JS is cells of `<case>.js` looked up by title.
/// Every other case runs its `.js` file cell by cell.
fn rust_case(case: &str) -> bool {
    [
        "missing",
        "close-preparing",
        "close-creating",
        "timeline-view",
        "piano-roll-cut-paste",
        "scope",
        "scope-master",
        "scope-load",
        "partial-run",
        "snippets",
        "tutorials",
        "script-ui",
        "patch-browse",
        "browser-change",
        "state-load",
        "dock",
        "dock-script-ui",
    ]
    .contains(&case)
}

pub fn run(case: &str) -> Result<(), String> {
    let rust = rust_case(case);
    let script = Script::load(if case == "scope-load" { "scope" } else { case }, !rust)?;
    let log = log::Log::new()?;
    let mut app = Control::open(log, &script.init)?;
    let mut result = (|| {
        if case == "close-preparing" {
            return Ok(());
        }
        while !app.ready {
            if let Err(error) = app.poll() {
                if case == "missing" && error == crate::i18n::t("install_cli_missing") {
                    app.log
                        .event("expected_error", json!({"stage":"catalog","error":error}))?;
                    return Ok(());
                }
                return Err(error);
            }
            if case == "close-creating" && app.creation_requested() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if case == "missing" {
            return Err("Missing target unexpectedly became Ready".into());
        }
        let app = &mut app;
        match case {
            _ if !rust => script.run(app, case),
            "timeline-view" => crate::timeline::regression::run(app, &script),
            "piano-roll-cut-paste" => crate::piano_roll_e2e::run(app, &script),
            "scope" => crate::scope_e2e::run(app, &script),
            "scope-master" => crate::scope_e2e::run_master(app, &script),
            "scope-load" => crate::scope_e2e::run_load(app, &script),
            "partial-run" => crate::execution_e2e::run(app, &script),
            "snippets" => crate::snippets_e2e::run(app, &script),
            "tutorials" => crate::tutorials_e2e::run(app, &script),
            "state-load" => state_load::run(app, &script),
            "patch-browse" => crate::patch_browse_e2e::run(app, &script),
            "browser-change" => crate::browser_change_e2e::run(app, &script),
            "script-ui" => crate::script_ui::regression::run(app, &script),
            "dock" | "dock-script-ui" => Ok(()), // Need the engine by value; run below.
            _ => Err(format!("Unhandled E2E case: {case}")),
        }
    })();
    if matches!(case, "dock" | "dock-script-ui") && result.is_ok() {
        let regression = match case {
            "dock" => crate::gui::dock_regression,
            _ => crate::gui::dock_script_ui_regression,
        };
        let (returned, dock) = regression(app, &script);
        app = returned;
        result = dock;
    }
    if let Err(error) = &result {
        app.log
            .event("error", json!({"stage":"E2E","error":error}))?;
    }
    app.close()?;
    if result.is_ok() {
        app.log
            .event("e2e_pass", json!({"case":case,"cleanup":"complete"}))?;
    }
    result
}

pub(crate) fn wait_stopped(app: &mut Control) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(10);
    while app.stopping {
        app.poll()?;
        if Instant::now() > until {
            return Err("Engine stop timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
