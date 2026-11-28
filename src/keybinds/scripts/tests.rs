use super::super::{embedded, parse as parse_file, SOURCE};
use super::*;
use eframe::egui::Key;

/// `keybinds.toml` without its `[scripts]` table, which ends the file.
fn commands() -> &'static str {
    let text = SOURCE.embedded();
    text.find("\n[scripts]").map_or(text, |end| &text[..end])
}

fn with_scripts(lines: &str) -> (Keybinds, Vec<String>) {
    parse_file(&format!("{}\n[scripts]\n{lines}", commands()))
}

fn press(key: Key, modifiers: Modifiers, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    }
}

/// Runs one frame with `events` and returns the bound keys pressed, and what is left.
fn pressed(bound: &[Binding], events: Vec<Event>) -> (Vec<String>, Vec<Event>) {
    let ctx = egui::Context::default();
    let mut result = (vec![], vec![]);
    ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| result = ui.input_mut(|input| (presses(bound, input), input.events.clone())),
    )
    .drop_without_applying_deltas();
    result
}

#[test]
fn scripts_table_is_read_as_keys_and_include_paths() {
    let (keybinds, errors) = with_scripts("\"Ctrl+P\" = \"builtin/play-loop.js\"");
    assert!(errors.is_empty(), "{errors:?}");
    let scripts: Vec<_> = keybinds
        .scripts
        .iter()
        .map(|(binding, path)| (binding.key, binding.modifiers, path.as_str()))
        .collect();
    assert_eq!(scripts, [(Key::P, Modifiers::CTRL, "builtin/play-loop.js")]);
    assert_eq!(keybinds.bindings, embedded().bindings);
    let config = config(&keybinds);
    assert_eq!(config["scripts"], json!({"Ctrl+P": "builtin/play-loop.js"}));
    assert_eq!(config["run_cell"], "Ctrl+Enter");
}

#[test]
fn unreadable_scripts_and_scripts_on_a_command_key_are_reported_and_left_out() {
    let (keybinds, errors) = with_scripts(
        "\"Ctrl+Nope\" = \"a.js\"\n\"F2\" = 3\n\"F3\" = \"\"\n\"F1\" = \"help.js\"\n\"F4\" = \"b.js\"",
    );
    assert_eq!(errors.len(), 4, "{errors:?}");
    let paths: Vec<_> = keybinds.scripts.iter().map(|(_, path)| path).collect();
    assert_eq!(paths, ["b.js"]);
    let (keybinds, errors) = parse_file(&format!("scripts = 1\n{}", commands()));
    assert_eq!((keybinds.scripts.len(), errors.len()), (0, 1), "{errors:?}");
}

#[test]
fn binding_a_command_key_or_an_unreadable_key_fails() {
    let keybinds = embedded();
    let mut bound = Vec::new();
    // Ctrl+Enter is run_cell's (editor only), F1 help's, Ctrl+Shift+P snippets'.
    for keys in [
        "Ctrl+Enter",
        "F1",
        "Ctrl+Shift+P",
        "Escape",
        "Ctrl+Nope",
        "",
    ] {
        assert!(bind(&mut bound, &keybinds, keys).is_err(), "{keys}");
    }
    assert!(bound.is_empty());
    assert_eq!(bind(&mut bound, &keybinds, "Ctrl+P"), Ok("Ctrl+P".into()));
}

#[test]
fn binding_a_key_again_keeps_one_entry_under_one_name() {
    let keybinds = embedded();
    let mut bound = Vec::new();
    let first = bind(&mut bound, &keybinds, "Shift+Ctrl+F2").unwrap();
    let second = bind(&mut bound, &keybinds, "Ctrl+Shift+F2").unwrap();
    assert_eq!(
        (first.as_str(), second.as_str()),
        ("Ctrl+Shift+F2", "Ctrl+Shift+F2")
    );
    assert_eq!(bound.len(), 1);
    assert_eq!(bind(&mut bound, &keybinds, "Ctrl++"), Ok("Ctrl++".into()));
    assert_eq!(bound.len(), 2);
}

#[test]
fn a_bound_key_fires_on_exact_modifiers_once_per_press() {
    let keybinds = embedded();
    let mut bound = Vec::new();
    bind(&mut bound, &keybinds, "Ctrl+P").unwrap();
    let ctrl = Modifiers::CTRL | Modifiers::COMMAND;
    let (names, left) = pressed(
        &bound,
        vec![
            press(Key::P, ctrl, false),
            press(Key::P, ctrl, true),
            Event::Text("P".into()),
        ],
    );
    assert_eq!((names, left), (vec!["Ctrl+P".to_owned()], vec![]));
    // One press per frame: egui marks a second press without a release as a repeat.
    for modifiers in [ctrl | Modifiers::SHIFT, Modifiers::NONE] {
        let other = vec![press(Key::P, modifiers, false)];
        assert_eq!(pressed(&bound, other.clone()), (vec![], other));
    }
}

#[test]
fn the_run_of_a_key_calls_its_function_by_name() {
    let request = script_request("Ctrl+P");
    assert_eq!(request.code, "__replRunKey(\"Ctrl+P\")");
    assert_eq!(request.scope, SCRIPT_SCOPE);
}
