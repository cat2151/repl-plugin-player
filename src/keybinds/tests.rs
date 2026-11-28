use super::*;

fn press(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Runs one frame with `events` and reports whether `command` fired, and what is left.
fn consumed(command: Command, events: Vec<Event>) -> (bool, Vec<Event>) {
    let ctx = egui::Context::default();
    let mut result = (false, vec![]);
    ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            result = ui.input_mut(|input| (consume(input, command), input.events.clone()));
        },
    )
    .drop_without_applying_deltas();
    result
}

#[test]
fn embedded_file_binds_every_command_once() {
    let (keybinds, errors) = parse(SOURCE.embedded());
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(keybinds.bindings.len(), Command::ALL.len());
}

#[test]
fn key_text_is_read_as_modifiers_and_one_key() {
    let read = |text| binding(text).map(|b| (b.key, b.modifiers));
    assert_eq!(read("F1"), Some((Key::F1, Modifiers::NONE)));
    assert_eq!(
        read("Ctrl+Shift+P"),
        Some((Key::P, Modifiers::CTRL | Modifiers::SHIFT))
    );
    assert_eq!(
        read("Ctrl+Alt+["),
        Some((Key::OpenBracket, Modifiers::CTRL | Modifiers::ALT))
    );
    assert_eq!(read("Ctrl++"), Some((Key::Plus, Modifiers::CTRL)));
    assert_eq!(read("+"), Some((Key::Plus, Modifiers::NONE)));
    for invalid in ["", "Ctrl+", "Control+P", "Ctrl+NoSuchKey", "ctrl+P"] {
        assert_eq!(read(invalid), None, "{invalid}");
    }
}

#[test]
fn invalid_missing_and_unknown_entries_keep_the_embedded_key_and_are_reported() {
    let text = SOURCE
        .embedded()
        .replace("run_cell = \"Ctrl+Enter\"", "run_cell = \"Ctrl+Nope\"")
        .replace("fold = \"Ctrl+.\"", "folding = \"Ctrl+.\"");
    let (keybinds, errors) = parse(&text);
    assert_eq!(errors.len(), 3, "{errors:?}");
    assert_eq!(keybinds, embedded());
}

#[test]
fn edited_entry_replaces_only_that_command() {
    let text = SOURCE.embedded().replace("help = \"F1\"", "help = \"F2\"");
    let (keybinds, errors) = parse(&text);
    assert!(errors.is_empty(), "{errors:?}");
    let help = &keybinds.bindings[Command::Help as usize];
    assert_eq!((help.key, help.text.as_str()), (Key::F2, "F2"));
    let unchanged = |bindings: &[Binding]| {
        let mut bindings = bindings.to_vec();
        bindings.remove(Command::Help as usize);
        bindings
    };
    assert_eq!(
        unchanged(&keybinds.bindings),
        unchanged(&embedded().bindings)
    );
}

#[test]
fn snippet_window_keys_default_to_the_former_fixed_keys() {
    let keybinds = embedded();
    let key = |command: Command| {
        let binding = &keybinds.bindings[command as usize];
        (binding.key, binding.modifiers)
    };
    let plain = |key: Key| (key, Modifiers::NONE);
    assert_eq!(
        [
            Command::SnippetFilter,
            Command::SnippetUp,
            Command::SnippetDown,
            Command::SnippetPageUp,
            Command::SnippetPageDown,
            Command::SnippetRun,
            Command::SnippetClose,
        ]
        .map(key),
        [
            Key::Slash,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::PageUp,
            Key::PageDown,
            Key::Enter,
            Key::Escape,
        ]
        .map(plain)
    );
}

#[test]
fn edited_snippet_window_entry_is_read() {
    let text = SOURCE
        .embedded()
        .replace("snippet_close = \"Escape\"", "snippet_close = \"F4\"");
    let (keybinds, errors) = parse(&text);
    assert!(errors.is_empty(), "{errors:?}");
    let close = &keybinds.bindings[Command::SnippetClose as usize];
    assert_eq!((close.key, close.text.as_str()), (Key::F4, "F4"));
}

#[test]
fn a_shared_key_is_reported_only_where_both_commands_listen_at_once() {
    let edit = |from: &str, to: &str| parse(&SOURCE.embedded().replace(from, to)).1;
    // Both in the Snippets window.
    let errors = edit("snippet_run = \"Enter\"", "snippet_run = \"ArrowUp\"");
    assert_eq!(errors.len(), 1, "{errors:?}");
    // help listens everywhere, so it also meets the Snippets window.
    let errors = edit("snippet_close = \"Escape\"", "snippet_close = \"F1\"");
    assert_eq!(errors.len(), 1, "{errors:?}");
    // The editor's keys listen only while the editor has focus.
    let errors = edit("fold = \"Ctrl+.\"", "fold = \"Enter\"");
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn shared_keys_and_broken_toml_are_reported() {
    let shared = SOURCE
        .embedded()
        .replace("help = \"F1\"", "help = \"Ctrl+,\"");
    assert_eq!(parse(&shared).1.len(), 1);
    let (keybinds, errors) = parse("[run_cell");
    assert_eq!((keybinds, errors.len()), (embedded(), 1));
}

#[test]
fn modifiers_must_match_exactly() {
    let windows_ctrl = Modifiers::CTRL | Modifiers::COMMAND;
    assert!(consumed(Command::RunCell, vec![press(Key::Enter, windows_ctrl)]).0);
    assert!(consumed(Command::RunCell, vec![press(Key::Enter, Modifiers::CTRL)]).0);
    for extra in [Modifiers::SHIFT, Modifiers::ALT] {
        let events = vec![press(Key::Enter, windows_ctrl | extra)];
        let (fired, left) = consumed(Command::RunCell, events.clone());
        assert!(!fired);
        assert_eq!(left, events);
    }
}

#[test]
fn the_character_of_a_fired_key_is_not_typed() {
    let events = vec![press(Key::Comma, Modifiers::CTRL), Event::Text(",".into())];
    assert_eq!(consumed(Command::Settings, events), (true, vec![]));
    let text = vec![Event::Text(",".into())];
    assert_eq!(consumed(Command::Settings, text.clone()), (false, text));
}

#[test]
fn editor_focus_page_and_window_keys_default_to_the_former_fixed_keys() {
    let keybinds = embedded();
    let key = |command: Command| {
        let binding = &keybinds.bindings[command as usize];
        (binding.key, binding.modifiers)
    };
    assert_eq!(
        [
            Command::FocusEditor,
            Command::PageUp,
            Command::PageDown,
            Command::CloseWindow,
        ]
        .map(key),
        [Key::Tab, Key::PageUp, Key::PageDown, Key::Escape].map(|key| (key, Modifiers::NONE))
    );
}

#[test]
fn shift_added_to_a_key_is_told_apart_and_taken_only_when_asked() {
    let extra = |modifiers| extra_modifiers(Command::PageDown, Key::PageDown, None, modifiers);
    assert_eq!(extra(Modifiers::NONE), Some(Modifiers::NONE));
    assert_eq!(extra(Modifiers::SHIFT), Some(Modifiers::SHIFT));
    for other in [
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers::SHIFT | Modifiers::CTRL,
    ] {
        assert_eq!(extra(other), None, "{other:?}");
    }
    // A key written with Shift takes no further Shift.
    let shifted = Modifiers::SHIFT;
    assert_eq!(
        extra_modifiers(Command::RunLine, Key::Enter, None, shifted),
        Some(Modifiers::NONE)
    );
    assert_eq!(
        extra_modifiers(Command::RunLine, Key::Enter, None, Modifiers::NONE),
        None
    );

    let tab = |modifiers| vec![press(Key::Tab, modifiers)];
    assert!(!consumed(Command::FocusEditor, tab(Modifiers::SHIFT)).0);
    let ctx = egui::Context::default();
    let mut fired = (false, false);
    ctx.run_ui(
        egui::RawInput {
            events: [tab(Modifiers::SHIFT), tab(Modifiers::NONE)].concat(),
            ..Default::default()
        },
        |ui| {
            fired = ui.input_mut(|input| {
                let first = consume_or_shifted(input, Command::FocusEditor);
                (first, input.events.is_empty())
            });
        },
    )
    .drop_without_applying_deltas();
    assert_eq!(fired, (true, true));
}

#[test]
fn keys_outside_the_editor_and_in_repl_ui_windows_are_checked_where_they_meet() {
    let edit = |from: &str, to: &str| parse(&SOURCE.embedded().replace(from, to)).1;
    // The Snippets window is open in the main window, where focus_editor listens.
    let errors = edit("focus_editor = \"Tab\"", "focus_editor = \"Enter\"");
    assert_eq!(errors.len(), 1, "{errors:?}");
    // focus_editor never listens while the editor has focus.
    let errors = edit("focus_editor = \"Tab\"", "focus_editor = \"Ctrl+Enter\"");
    assert!(errors.is_empty(), "{errors:?}");
    // A repl.ui window of its own has its own input.
    let errors = edit("close_window = \"Escape\"", "close_window = \"Enter\"");
    assert!(errors.is_empty(), "{errors:?}");
    let errors = edit("close_window = \"Escape\"", "close_window = \"F1\"");
    assert_eq!(errors.len(), 1, "{errors:?}");
}
