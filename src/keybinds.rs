//! `keybinds.toml`: the key of each [`Command`]. Read once by [`load`] at startup; an
//! invalid or missing entry keeps the embedded key and is reported as a message.
use crate::resources::Source;
use eframe::egui::{self, Event, Key, Modifiers};
use std::sync::OnceLock;

mod layout;
#[cfg(all(windows, test))]
pub(crate) use layout::layout_character;
mod scripts;
pub use scripts::{dispatch, script_request, take_script_presses, SCRIPT_SCOPE};

static SOURCE: Source = Source::new("keybinds.toml", include_str!("../keybinds.toml"));
static KEYBINDS: OnceLock<Keybinds> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    RunCell,
    RunLine,
    PreviousCell,
    NextCell,
    Fold,
    Snippets,
    Settings,
    Help,
    SnippetFilter,
    SnippetUp,
    SnippetDown,
    SnippetPageUp,
    SnippetPageDown,
    SnippetRun,
    SnippetClose,
    FocusEditor,
    PageUp,
    PageDown,
    CloseWindow,
}

/// Where a command's key is listened for. Two commands may share a key only when
/// they never listen at the same time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Listened for whatever has focus.
    Everywhere,
    /// While the code editor has focus.
    Editor,
    /// While the Snippets window is open.
    SnippetWindow,
    /// In the main window while the code editor does not have focus.
    OutsideEditor,
    /// While a `repl.ui` window of its own has focus.
    ScriptWindow,
}

impl Scope {
    fn overlaps(self, other: Self) -> bool {
        let outside = |a, b| a == Self::OutsideEditor && b == Self::SnippetWindow;
        self == other
            || self == Self::Everywhere
            || other == Self::Everywhere
            || outside(self, other)
            || outside(other, self)
    }
}

impl Command {
    const ALL: [Self; 19] = [
        Self::RunCell,
        Self::RunLine,
        Self::PreviousCell,
        Self::NextCell,
        Self::Fold,
        Self::Snippets,
        Self::Settings,
        Self::Help,
        Self::SnippetFilter,
        Self::SnippetUp,
        Self::SnippetDown,
        Self::SnippetPageUp,
        Self::SnippetPageDown,
        Self::SnippetRun,
        Self::SnippetClose,
        Self::FocusEditor,
        Self::PageUp,
        Self::PageDown,
        Self::CloseWindow,
    ];

    fn scope(self) -> Scope {
        match self {
            Self::RunCell
            | Self::RunLine
            | Self::PreviousCell
            | Self::NextCell
            | Self::Fold
            | Self::PageUp
            | Self::PageDown => Scope::Editor,
            Self::Snippets | Self::Settings | Self::Help => Scope::Everywhere,
            Self::SnippetFilter
            | Self::SnippetUp
            | Self::SnippetDown
            | Self::SnippetPageUp
            | Self::SnippetPageDown
            | Self::SnippetRun
            | Self::SnippetClose => Scope::SnippetWindow,
            Self::FocusEditor => Scope::OutsideEditor,
            Self::CloseWindow => Scope::ScriptWindow,
        }
    }

    /// The key of this command in `keybinds.toml`.
    fn name(self) -> &'static str {
        match self {
            Self::RunCell => "run_cell",
            Self::RunLine => "run_line",
            Self::PreviousCell => "previous_cell",
            Self::NextCell => "next_cell",
            Self::Fold => "fold",
            Self::Snippets => "snippets",
            Self::Settings => "settings",
            Self::Help => "help",
            Self::SnippetFilter => "snippet_filter",
            Self::SnippetUp => "snippet_up",
            Self::SnippetDown => "snippet_down",
            Self::SnippetPageUp => "snippet_page_up",
            Self::SnippetPageDown => "snippet_page_down",
            Self::SnippetRun => "snippet_run",
            Self::SnippetClose => "snippet_close",
            Self::FocusEditor => "focus_editor",
            Self::PageUp => "page_up",
            Self::PageDown => "page_down",
            Self::CloseWindow => "close_window",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Binding {
    key: Key,
    modifiers: Modifiers,
    /// As written in the file; the help page shows it.
    text: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Keybinds {
    /// Indexed by `Command as usize`.
    bindings: Vec<Binding>,
    /// `[scripts]`: a key and the `include` path it runs, in the file's order.
    scripts: Vec<(Binding, String)>,
}

impl Keybinds {
    /// The command whose key is `binding`'s. A script key listens everywhere, so
    /// it meets every command.
    fn command_of(&self, binding: &Binding) -> Option<Command> {
        Command::ALL
            .into_iter()
            .zip(&self.bindings)
            .find(|(_, other)| other.key == binding.key && other.modifiers == binding.modifiers)
            .map(|(command, _)| command)
    }
}

/// Reads the file once; later calls keep the first result and report nothing.
pub fn load() -> Vec<String> {
    let mut errors = Vec::new();
    KEYBINDS.get_or_init(|| {
        let (keybinds, found) = read();
        errors = found;
        keybinds
    });
    errors
}

fn read() -> (Keybinds, Vec<String>) {
    match SOURCE.text() {
        Ok(text) => parse(text),
        Err(error) => (embedded(), vec![read_error(error)]),
    }
}

fn get() -> &'static Keybinds {
    KEYBINDS.get_or_init(|| read().0)
}

/// The key as written in `keybinds.toml`.
pub fn text(command: Command) -> &'static str {
    &get().bindings[command as usize].text
}

/// When a press is `command`'s key, the modifiers held beyond it: none, or Shift
/// added to a key written without it.
pub fn extra_modifiers(
    command: Command,
    key: Key,
    physical_key: Option<Key>,
    modifiers: Modifiers,
) -> Option<Modifiers> {
    binding_extra(
        &get().bindings[command as usize],
        key,
        physical_key,
        modifiers,
    )
}

fn binding_extra(
    binding: &Binding,
    key: Key,
    physical_key: Option<Key>,
    modifiers: Modifiers,
) -> Option<Modifiers> {
    if layout::logical(key, physical_key) != Some(binding.key) {
        return None;
    }
    let unshifted = Modifiers {
        shift: false,
        ..modifiers
    };
    if modifiers.matches_exact(binding.modifiers) {
        Some(Modifiers::NONE)
    } else if modifiers.shift
        && !binding.modifiers.shift
        && unshifted.matches_exact(binding.modifiers)
    {
        Some(Modifiers::SHIFT)
    } else {
        None
    }
}

/// Consumes the presses of `command`'s key, and the text that key would type.
pub fn consume(input: &mut egui::InputState, command: Command) -> bool {
    consume_with(input, command, false)
}

/// [`consume`], also taking the key with Shift added.
pub fn consume_or_shifted(input: &mut egui::InputState, command: Command) -> bool {
    consume_with(input, command, true)
}

fn consume_with(input: &mut egui::InputState, command: Command, shifted: bool) -> bool {
    let binding = &get().bindings[command as usize];
    let mut pressed = false;
    input.events.retain(|event| {
        let matched = matches!(event, Event::Key { key, physical_key, modifiers, pressed: true, .. }
            if extra_modifiers(command, *key, *physical_key, *modifiers)
                .is_some_and(|extra| shifted || extra.is_none()));
        pressed |= matched;
        !matched
    });
    if pressed {
        drop_typed(input, binding.key);
    }
    pressed
}

/// Removes the text that pressing `key` would type.
fn drop_typed(input: &mut egui::InputState, key: Key) {
    let typed = key.symbol_or_name();
    input
        .events
        .retain(|event| !matches!(event, Event::Text(text) if text == typed));
}

fn embedded() -> Keybinds {
    let (keybinds, errors) = parse(SOURCE.embedded());
    assert!(errors.is_empty(), "embedded keybinds.toml: {errors:?}");
    keybinds
}

fn read_error(error: impl ToString) -> String {
    crate::i18n::render("keybinds_read_error", &[("error", error.to_string())])
}

fn parse(text: &str) -> (Keybinds, Vec<String>) {
    let mut errors = Vec::new();
    let table: toml::Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(error) => return (embedded(), vec![read_error(error)]),
    };
    for name in table.keys() {
        if name != scripts::TABLE && !Command::ALL.iter().any(|command| command.name() == name) {
            errors.push(crate::i18n::render(
                "keybinds_unknown",
                &[("command", name.clone())],
            ));
        }
    }
    let mut fallback = None;
    let mut bindings = Vec::new();
    for command in Command::ALL {
        let name = command.name();
        let value = table.get(name);
        match value.and_then(toml::Value::as_str).map(binding) {
            Some(Some(binding)) => bindings.push(binding),
            _ => {
                let default =
                    fallback.get_or_insert_with(embedded).bindings[command as usize].clone();
                let written = value.map_or_else(|| "\"\"".into(), ToString::to_string);
                errors.push(crate::i18n::render(
                    "keybinds_invalid",
                    &[
                        ("command", name.into()),
                        ("keys", written),
                        ("default", default.text.clone()),
                    ],
                ));
                bindings.push(default);
            }
        }
    }
    for (index, first) in bindings.iter().enumerate() {
        for (second, other) in Command::ALL.iter().zip(&bindings).skip(index + 1) {
            if first.key == other.key
                && first.modifiers == other.modifiers
                && Command::ALL[index].scope().overlaps(second.scope())
            {
                errors.push(crate::i18n::render(
                    "keybinds_duplicate",
                    &[
                        ("first", Command::ALL[index].name().into()),
                        ("second", second.name().into()),
                        ("keys", first.text.clone()),
                    ],
                ));
            }
        }
    }
    let mut keybinds = Keybinds {
        bindings,
        scripts: Vec::new(),
    };
    keybinds.scripts = scripts::parse(&keybinds, table.get(scripts::TABLE), &mut errors);
    (keybinds, errors)
}

/// The press or release of `text`'s key with its modifiers, as egui on Windows
/// sends it (Ctrl also sets `command`).
pub(crate) fn key_event(text: &str, pressed: bool) -> Option<Event> {
    let Binding { key, modifiers, .. } = binding(text)?;
    Some(Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers {
            command: modifiers.ctrl,
            ..modifiers
        },
    })
}

/// `Ctrl+Shift+P`, `F1`, `Ctrl++`: modifiers then one egui key name, joined by `+`.
fn binding(text: &str) -> Option<Binding> {
    let (prefix, name) = match text.strip_suffix("++") {
        Some(prefix) => (prefix, "+"),
        None if text == "+" => ("", "+"),
        None => text.rsplit_once('+').unwrap_or(("", text)),
    };
    let key = Key::from_name(name)?;
    let mut modifiers = Modifiers::NONE;
    for part in prefix.split('+').filter(|_| !prefix.is_empty()) {
        match part {
            "Ctrl" => modifiers.ctrl = true,
            "Shift" => modifiers.shift = true,
            "Alt" => modifiers.alt = true,
            _ => return None,
        }
    }
    Some(Binding {
        key,
        modifiers,
        text: text.into(),
    })
}

#[cfg(test)]
mod tests;
