//! The user's patch browser flow against a stand-in player that offers real
//! catalog states: while a loop plays, the browse cell auditions each patch on
//! the loop's track and closing brings back the track's own instrument; the
//! cell written for the last patch then makes it the track's instrument. A
//! second browse auditions on that replaced track.
//! Test-owned cells: `tests/e2e/patch-browse.js`.
use crate::{
    control::Control,
    e2e_script::Script,
    host::PluginKey,
    patch_browser::Link,
    resident_ipc::{self, Open, Selected, ToEditor, ToPlayer},
};
use serde_json::{json, Map, Value};
use std::{
    collections::VecDeque,
    path::Path,
    process::Child,
    sync::Mutex,
    time::{Duration, Instant},
};

const SURGE: (&str, &str) = ("Surge XT", "org.surge-synth-team.surge-xt");
const DEXED: (&str, &str) = ("Dexed", "com.digital-suburban.dexed");
const ATTACKY: &str = "patches_factory/Basses/Attacky.fxp";
const LAVITAR: &str = "SynprezFM/SynprezFM_14.syx/09 Lavitar";
const BELL_DREAMS: &str = "SynprezFM/SynprezFM_14.syx/07 BellDreams";

/// What the stand-in offers after each `Open`: the plugin, the display path and its state.
type Offer = (PluginKey, String, Vec<u8>);
static BATCHES: Mutex<VecDeque<Vec<Offer>>> = Mutex::new(VecDeque::new());
/// What the stand-in received in answer to its offers.
static ANSWERS: Mutex<Vec<ToPlayer>> = Mutex::new(Vec::new());

fn start() -> Result<Link, String> {
    Link::start(Path::new("stand-in"), launch)
}

fn launch(_: &Path, name: &str) -> std::io::Result<Option<Child>> {
    let name = name.to_owned();
    std::thread::spawn(move || stand_in(&name));
    Ok(None)
}

/// At each `Open`, offers the next batch, each after the last was answered,
/// then hides the browser; stays until the app drops the link.
fn stand_in(name: &str) {
    let Ok((mut out, mut input)) = resident_ipc::connect(name) else {
        return;
    };
    while let Ok(Some(message)) = input.receive::<ToPlayer>() {
        if !matches!(message, ToPlayer::Open(_)) {
            continue;
        }
        let offers = BATCHES.lock().unwrap().pop_front().unwrap_or_default();
        for (plugin, patch, bytes) in offers {
            let Ok(shared) = out.share(&bytes) else {
                return;
            };
            let selected = Selected {
                kit: None,
                reference: None,
                target: Open::Patch { track: 0 },
                plugin,
                preset_name: Some(patch),
                state: shared.reference.clone(),
            };
            if out.send(&ToEditor::Selected(selected)).is_err() {
                return;
            }
            match input.receive::<ToPlayer>() {
                Ok(Some(answer)) => ANSWERS.lock().unwrap().push(answer),
                _ => return,
            }
        }
        let hidden = ToEditor::BrowserClosed {
            target: Open::Patch { track: 0 },
        };
        if out.send(&hidden).is_err() {
            return;
        }
    }
}

fn offer((name, id): (&str, &str), patch: &str) -> Result<Offer, String> {
    let file = crate::patch_state::file("CLAP", id, patch)?;
    let plugin = PluginKey {
        format: "CLAP".into(),
        id: id.into(),
        name: name.into(),
        vendor: String::new(),
        bundle_path: String::new(),
    };
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    Ok((plugin, patch.to_owned(), bytes))
}

fn changed(before: &Value, after: &Value) -> usize {
    let empty = Map::new();
    let before = before.as_object().unwrap_or(&empty);
    after
        .as_object()
        .unwrap_or(&empty)
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(v))
        .count()
}

fn id(value: &Value) -> i64 {
    // JSON from the script may write an integer as `0.0`.
    value.as_f64().map_or(-1, |id| id as i64)
}

fn ids(plugins: &Value) -> Vec<(i64, bool)> {
    let mut found: Vec<_> = plugins
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            (
                id(&p["instanceId"]),
                p["bypassed"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    found.sort();
    found
}

const AUDIBLE: f64 = 0.001;

/// Runs the browse cell on track 0 while the stand-in offers `offers`, and
/// checks that each audition sounds alone beside the bypassed instrument and
/// that closing brings back the instrument untouched. Returns the record.
fn browse(app: &mut Control, script: &Script, offers: Vec<Offer>) -> Result<Value, String> {
    let count = offers.len();
    BATCHES.lock().unwrap().push_back(offers);
    ANSWERS.lock().unwrap().clear();
    app.eval(script.cell("probe")?, "patch-browse probe")?;
    let ran = app.eval(script.cell("browse")?, "patch-browse cell");
    let result: Value =
        serde_json::from_str(&app.eval(script.cell("result")?, "patch-browse result")?)
            .map_err(|e| e.to_string())?;
    ran?;
    let answers = std::mem::take(&mut *ANSWERS.lock().unwrap());
    if answers != vec![ToPlayer::Applied(Ok(())); count] {
        return Err(format!("The stand-in got {answers:?}: {result}"));
    }
    let original = id(&result["original"]);
    let fail = |what: &str| Err(format!("{what}: {result}"));
    if original < 0
        || ids(&result["before"]["plugins"]) != [(original, false)]
        || result["before"]["peak"].as_f64() < Some(AUDIBLE)
    {
        return fail("The loop did not play on the track's instrument");
    }
    let heard = result["answers"].as_array().cloned().unwrap_or_default();
    let alone = heard.len() == count
        && heard.iter().all(|a| {
            let plugins = ids(&a["plugins"]);
            plugins.len() == 2
                && plugins.contains(&(original, true))
                && plugins.iter().any(|p| p.0 != original && !p.1)
        });
    if !alone {
        return fail("Each audition is not one plugin beside the bypassed instrument");
    }
    if heard.iter().any(|a| a["peak"].as_f64() < Some(AUDIBLE)) {
        return fail("An audition was silent");
    }
    if ids(&result["after"]["plugins"]) != [(original, false)]
        || changed(
            &result["before"]["parameters"],
            &result["after"]["parameters"],
        ) != 0
        || result["before"]["connections"] != result["after"]["connections"]
        || result["after"]["playing"] != true
        || result["after"]["peak"].as_f64() < Some(AUDIBLE)
    {
        return fail("Closing did not bring back the track's own sound");
    }
    Ok(result)
}

/// The cell written for the last patch names the plugin, the patch and the browsed track.
fn written_cell(script: &Script, (name, _): (&str, &str), patch: &str) -> Result<String, String> {
    let cell = crate::patch_browse::take_cell().map(|s| s.code);
    let expected = script
        .cell("written cell")?
        .replace("PLUGIN", name)
        .replace("PATCH", patch);
    if cell.as_deref() != Some(expected.as_str()) || crate::patch_browse::take_cell().is_some() {
        return Err(format!("Not one cell for the last patch: {cell:?}"));
    }
    Ok(expected)
}

/// Runs the written cell; from a later evaluation (dispose is asynchronous),
/// track 0 holds one instance of the plugin, with the parameters it had while
/// auditioned. Returns that instance.
fn ran_cell(
    app: &mut Control,
    script: &Script,
    cell: &str,
    (name, plugin_id): (&str, &str),
    auditioned: &Value,
) -> Result<i64, String> {
    app.eval(cell, "patch-browse written cell")?;
    let code = script.cell("plugins on track 0")?;
    let now: Value = serde_json::from_str(&app.eval(code, "patch-browse track 0")?)
        .map_err(|e| e.to_string())?;
    let instance = match now.as_array().map(Vec::as_slice) {
        Some([only]) if only[1] == plugin_id => id(&only[0]),
        _ => return Err(format!("Track 0 is not one {name} after the cell: {now}")),
    };
    // A plugin may apply a loaded state on its audio thread.
    let deadline = Instant::now() + Duration::from_secs(3);
    let code = script.call("parameters", &json!({ "instance": instance }))?;
    loop {
        let values: Value = serde_json::from_str(&app.eval(&code, "patch-browse parameters")?)
            .map_err(|e| e.to_string())?;
        let differing = changed(auditioned, &values);
        if differing == 0 {
            return Ok(instance);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{name} on track 0 differs from its audition in {differing} parameters"
            ));
        }
        app.poll()?;
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn run(app: &mut Control, script: &Script) -> Result<(), String> {
    crate::patch_browse::start_with(start);
    app.eval(script.cell("loop")?, "patch-browse loop cell")?;
    let started = Instant::now();
    let first = browse(
        app,
        script,
        vec![
            offer(SURGE, ATTACKY)?,
            offer(DEXED, LAVITAR)?,
            offer(DEXED, BELL_DREAMS)?,
        ],
    )?;
    let elapsed = started.elapsed();
    let fail = |what: &str| Err(format!("{what}: {first}"));
    let [attacky, lavitar, bell] = first["answers"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    else {
        return fail("Not one record per offer");
    };
    let audition = |a: &Value| {
        ids(&a["plugins"])
            .into_iter()
            .find(|p| p.0 != id(&first["original"]))
            .map(|p| p.0)
    };
    if audition(lavitar) != audition(bell) || audition(attacky) == audition(lavitar) {
        return fail("A plugin's patches did not share one audition instance");
    }
    let dexed_changed = changed(&lavitar["parameters"], &bell["parameters"]);
    let surge_changed = changed(&first["before"]["parameters"], &attacky["parameters"]);
    if dexed_changed == 0 || surge_changed == 0 {
        return fail("An audition did not change the sound");
    }
    let notes = heard_notes(app)?;

    let cell = written_cell(script, DEXED, BELL_DREAMS)?;
    let dexed = ran_cell(app, script, &cell, DEXED, &bell["parameters"])?;
    // The replaced track's connections were rewired; auditions there sound too.
    let second = browse(app, script, vec![offer(SURGE, ATTACKY)?])?;
    if id(&second["original"]) != dexed {
        return Err(format!(
            "The second browse did not start from {dexed}: {second}"
        ));
    }
    let cell = written_cell(script, SURGE, ATTACKY)?;
    let surge = ran_cell(app, script, &cell, SURGE, &attacky["parameters"])?;
    app.log.event(
        "signal_check",
        json!({"patchBrowse":true,"dexedChanged":dexed_changed,"surgeChanged":surge_changed,
            "peaks":[attacky["peak"],lavitar["peak"],bell["peak"],first["after"]["peak"]],
            "replacedPeak":second["answers"][0]["peak"],
            "replacedConnections":second["after"]["connections"],"cellInstruments":[dexed,surge],
            "notesAfter":notes,"browseMs":elapsed.as_millis()}),
    )?;
    Ok(())
}

/// The restored instrument receives the loop's notes again.
fn heard_notes(app: &mut Control) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let notes = app.read_notes()?;
        if !notes.is_empty() {
            return Ok(notes);
        }
        if Instant::now() >= deadline {
            return Err("The restored instrument receives no note".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
