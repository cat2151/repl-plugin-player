use super::*;
use crate::{
    patch_browser::tests::{last_launched, record, selected},
    resident_ipc::{Open, ToEditor, ToPlayer},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn start() -> Result<Link, String> {
    Link::start(Path::new("player.exe"), record)
}

fn call(api: &str, input: Value) -> Value {
    dispatch(api, &input).unwrap()
}

fn next_events() -> Vec<Value> {
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(60), "timed out");
        let Value::Array(events) = call("patchBrowser.poll", json!({})) else {
            panic!("poll returns an array");
        };
        if !events.is_empty() {
            return events;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A player offering each patch with its state, then hiding the browser;
/// returns every message the app sent it.
fn player(name: String, patches: &'static [(&'static str, &'static [u8])]) -> Vec<ToPlayer> {
    let (mut out, mut input) = resident_ipc::connect(&name).unwrap();
    let mut received = vec![input.receive::<ToPlayer>().unwrap().unwrap()];
    for (patch, bytes) in patches {
        let shared = out.share(bytes).unwrap();
        let mut offer = selected(patch);
        offer.state = shared.reference.clone();
        out.send(&ToEditor::Selected(offer)).unwrap();
        received.push(input.receive::<ToPlayer>().unwrap().unwrap());
        drop(shared);
    }
    out.send(&ToEditor::BrowserClosed {
        target: Open::Patch { track: 0 },
    })
    .unwrap();
    received.push(input.receive::<ToPlayer>().unwrap().unwrap());
    out.send(&ToEditor::BrowserClosed {
        target: Open::Patch { track: 0 },
    })
    .unwrap();
    received
}

#[test]
fn selections_bring_their_state_and_the_last_accepted_one_becomes_one_cell() {
    start_with(start);
    call("patchBrowser.open", json!({"track":2}));
    const PATCHES: &[(&str, &[u8])] =
        &[("a/Attacky.fxp", b"first state"), ("b/Bad.fxp", b"second")];
    let name = last_launched();
    let player = std::thread::spawn(move || player(name, PATCHES));

    for (index, (patch, bytes)) in PATCHES.iter().enumerate() {
        let events = next_events();
        let [event] = events.as_slice() else {
            panic!("{events:?}");
        };
        assert_eq!(event["kind"], "selected");
        assert_eq!(event["format"], "CLAP");
        assert_eq!(event["pluginId"], "org.surge-synth-team.surge-xt");
        assert_eq!(event["patch"], *patch);
        let file = event["stateFile"].as_str().unwrap();
        assert_eq!(std::fs::read(file).unwrap(), *bytes);
        // The second one fails in the script and must not become the cell.
        let error = if index == 0 {
            json!(null)
        } else {
            json!("failed")
        };
        call("patchBrowser.answer", json!({ "error": error }));
    }
    assert_eq!(next_events(), vec![json!({"kind":"closed","error":null})]);
    let cell = take_cell().expect("one cell");
    assert_eq!(
        cell.code,
        "{\n    const track = 2;\n    include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', patch:'a/Attacky.fxp', track});\n}"
    );
    assert!(take_cell().is_none());

    // Browsing again asks the same player; nothing chosen, no cell.
    call("patchBrowser.open", json!({"track":0}));
    // The stand-in ends right after hiding, which may close the link in the same poll.
    assert_eq!(next_events()[0], json!({"kind":"closed","error":null}));
    assert!(take_cell().is_none());
    let open = ToPlayer::Open(Open::Patch { track: 0 });
    assert_eq!(
        player.join().unwrap(),
        vec![
            open.clone(),
            ToPlayer::Applied(Ok(())),
            ToPlayer::Applied(Err("failed".into())),
            open
        ]
    );
}

#[test]
fn opening_needs_the_browsed_track() {
    assert_eq!(
        dispatch("patchBrowser.open", &json!({})),
        Err(t("patch_browse_02").to_owned())
    );
}

#[test]
fn polling_without_a_browser_reports_it_closed_with_an_error() {
    assert_eq!(
        call("patchBrowser.poll", json!({})),
        json!([{"kind":"closed","error":t("patch_browse_01")}])
    );
    assert!(dispatch("patchBrowser.answer", &json!({"error":null})).is_err());
}
