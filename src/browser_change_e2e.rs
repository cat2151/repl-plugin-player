//! The browsers that change a track at once, against a stand-in player that
//! offers real states: `patch.js` and `effect.js` `change` leave the chosen
//! preset on the track after the browser closes, open the browser of their kind,
//! and write the last preset's cell unless opened with `cell:false`; the effect
//! cell restores the preset when run. The easy layer's `changePatch` and
//! `changeEffect` write no cell.
//! Test-owned cells: `tests/e2e/browser-change.js`.
use crate::{
    control::Control,
    e2e_script::Script,
    host::PluginKey,
    patch_browser::Link,
    resident_ipc::{self, Open, Selected, ToEditor, ToPlayer},
};
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path, process::Child, sync::Mutex};

const DEXED: (&str, &str) = ("Dexed", "com.digital-suburban.dexed");
const LAVITAR: &str = "SynprezFM/SynprezFM_14.syx/09 Lavitar";
const SURGE: (&str, &str) = ("Surge XT", "org.surge-synth-team.surge-xt");
const ATTACKY: &str = "patches_factory/Basses/Attacky.fxp";
const SURGE_FX: (&str, &str) = ("Surge XT Effects", "org.surge-synth-team.surge-xt-fx");
const CHAMBER: &str = "Airwindows/Ambience/Chamber.srgfx";

/// One offer for the stand-in to send after the next `Open`.
static OFFERS: Mutex<VecDeque<Selected>> = Mutex::new(VecDeque::new());
static STATES: Mutex<VecDeque<Vec<u8>>> = Mutex::new(VecDeque::new());
/// The browsers the app asked for, and the answers it gave.
static OPENED: Mutex<Vec<Open>> = Mutex::new(Vec::new());
static ANSWERS: Mutex<Vec<ToPlayer>> = Mutex::new(Vec::new());

fn launch(_: &Path, name: &str) -> std::io::Result<Option<Child>> {
    let name = name.to_owned();
    std::thread::spawn(move || stand_in(&name));
    Ok(None)
}

/// At each `Open`, offers the next selection, waits for its answer, then hides
/// the browser; stays until the app drops the link.
fn stand_in(name: &str) {
    let Ok((mut out, mut input)) = resident_ipc::connect(name) else {
        return;
    };
    while let Ok(Some(message)) = input.receive::<ToPlayer>() {
        let ToPlayer::Open(target) = message else {
            continue;
        };
        OPENED.lock().unwrap().push(target);
        let offer = OFFERS.lock().unwrap().pop_front();
        let bytes = STATES.lock().unwrap().pop_front();
        if let (Some(mut selected), Some(bytes)) = (offer, bytes) {
            let Ok(shared) = out.share(&bytes) else {
                return;
            };
            selected.target = target;
            selected.state = shared.reference.clone();
            if out.send(&ToEditor::Selected(selected)).is_err() {
                return;
            }
            match input.receive::<ToPlayer>() {
                Ok(Some(answer)) => ANSWERS.lock().unwrap().push(answer),
                _ => return,
            }
        }
        if out.send(&ToEditor::BrowserClosed { target }).is_err() {
            return;
        }
    }
}

fn key((name, id): (&str, &str), bundle_path: String) -> PluginKey {
    PluginKey {
        format: "CLAP".into(),
        id: id.into(),
        name: name.into(),
        vendor: String::new(),
        bundle_path,
    }
}

fn unused() -> resident_ipc::StateRef {
    resident_ipc::StateRef {
        name: String::new(),
        len: 0,
    }
}

fn patch(plugin: (&str, &str), patch: &str) -> Result<(), String> {
    let bytes = std::fs::read(crate::patch_state::file("CLAP", plugin.1, patch)?)
        .map_err(|e| e.to_string())?;
    OFFERS.lock().unwrap().push_back(Selected {
        kit: None,
        reference: None,
        target: Open::Patch { track: 0 },
        plugin: key(plugin, String::new()),
        preset_name: Some(patch.into()),
        state: unused(),
    });
    STATES.lock().unwrap().push_back(bytes);
    Ok(())
}

/// The effect as the player sends it: with its reference to the preset file.
fn effect(plugin: (&str, &str), preset: &str) -> Result<(), String> {
    let player = crate::patch_browser::player()?;
    let reference =
        crate::patch_state::effect_reference(&player, "CLAP", plugin.1, preset, preset)?;
    let bytes = std::fs::read(crate::patch_state::effect_file(
        "CLAP", plugin.1, preset, preset,
    )?)
    .map_err(|e| e.to_string())?;
    OFFERS.lock().unwrap().push_back(Selected {
        kit: None,
        target: Open::Effect {
            track: 0,
            slot: None,
        },
        plugin: key(plugin, reference.plugin.bundle_path.clone()),
        preset_name: Some(preset.into()),
        reference: Some(Box::new(reference)),
        state: unused(),
    });
    STATES.lock().unwrap().push_back(bytes);
    Ok(())
}

/// Runs the cell `title` against one offer; checks that the stand-in got one
/// `Open` of `kind` and one accepting answer, and that the track then holds
/// `plugin` as `role` ('instrument' or 'effects'). Returns the written cell.
fn change(
    app: &mut Control,
    script: &Script,
    title: &str,
    kind: &str,
    (role, plugin): (&str, (&str, &str)),
) -> Result<Option<String>, String> {
    OPENED.lock().unwrap().clear();
    ANSWERS.lock().unwrap().clear();
    app.eval(script.cell(title)?, title)?;
    let opened = std::mem::take(&mut *OPENED.lock().unwrap());
    let answers = std::mem::take(&mut *ANSWERS.lock().unwrap());
    let wanted = matches!(
        (kind, opened.as_slice()),
        ("patch", [Open::Patch { .. }]) | ("effect", [Open::Effect { .. }])
    );
    if !wanted || answers != [ToPlayer::Applied(Ok(()))] {
        return Err(format!("{title}: opened {opened:?}, answered {answers:?}"));
    }
    let now: Value = serde_json::from_str(&app.eval(script.cell("track 0")?, "track 0")?)
        .map_err(|e| e.to_string())?;
    let held = match role {
        "instrument" => now["instrument"] == plugin.1,
        _ => now["effects"] == json!([plugin.1]),
    };
    if !held {
        return Err(format!(
            "{title}: track 0 holds {now}, not {} as {role}",
            plugin.0
        ));
    }
    Ok(crate::patch_browse::take_cell().map(|s| s.code))
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    crate::patch_browse::start_with(|| Link::start(Path::new("stand-in"), launch));
    app.eval(script.cell("loop")?, "loop")?;

    patch(DEXED, LAVITAR)?;
    let cell = change(app, script, "change patch", "patch", ("instrument", DEXED))?;
    if !cell.is_some_and(|c| c.contains("patch.js').use(") && c.contains(LAVITAR)) {
        return Err("change patch wrote no cell for the patch".into());
    }
    patch(SURGE, ATTACKY)?;
    let cell = change(
        app,
        script,
        "easy changePatch",
        "patch",
        ("instrument", SURGE),
    )?;
    if cell.is_some() {
        return Err(format!("changePatch wrote a cell: {cell:?}"));
    }

    effect(SURGE_FX, CHAMBER)?;
    let cell = change(
        app,
        script,
        "change effect",
        "effect",
        ("effects", SURGE_FX),
    )?;
    let expected = script.cell("written effect cell")?;
    if cell.as_deref() != Some(expected) {
        return Err(format!("change effect wrote {cell:?}"));
    }
    app.eval(expected, "written effect cell")?;
    effect(SURGE_FX, CHAMBER)?;
    let cell = change(
        app,
        script,
        "easy changeEffect",
        "effect",
        ("effects", SURGE_FX),
    )?;
    if cell.is_some() {
        return Err(format!("changeEffect wrote a cell: {cell:?}"));
    }
    app.log
        .event("signal_check", json!({"browserChange":true}))?;
    Ok(())
}
