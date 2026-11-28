//! `[scripts]` of `keybinds.toml`, and the keys `repl.keys.bind` gives a script
//! function. Rust only names the pressed key; the function stays in JavaScript.
use super::{binding, binding_extra, drop_typed, get, Binding, Command, Keybinds};
use eframe::egui::{self, Event, Modifiers};
use serde_json::{json, Map, Value};
use std::sync::Mutex;

pub(super) const TABLE: &str = "scripts";
/// The scope of the run a bound key starts.
pub const SCRIPT_SCOPE: &str = "key";

static BOUND: Mutex<Vec<Binding>> = Mutex::new(Vec::new());

fn render(key: &str, values: &[(&str, &str)]) -> String {
    let values: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, value.to_string()))
        .collect();
    crate::i18n::render(key, &values)
}

/// The `[scripts]` entries whose key reads and is no command's key.
pub(super) fn parse(
    keybinds: &Keybinds,
    value: Option<&toml::Value>,
    errors: &mut Vec<String>,
) -> Vec<(Binding, String)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(table) = value.as_table() else {
        errors.push(crate::i18n::t("keybinds_scripts_table").into());
        return Vec::new();
    };
    let mut scripts = Vec::new();
    for (keys, path) in table {
        let read = binding(keys).zip(path.as_str().filter(|path| !path.is_empty()));
        let Some((binding, path_text)) = read else {
            let written = path.to_string();
            errors.push(render(
                "keybinds_script_invalid",
                &[("keys", keys), ("path", &written)],
            ));
            continue;
        };
        if let Some(command) = keybinds.command_of(&binding) {
            errors.push(render(
                "keybinds_duplicate",
                &[("first", TABLE), ("second", command.name()), ("keys", keys)],
            ));
            continue;
        }
        scripts.push((binding, path_text.to_owned()));
    }
    scripts
}

/// `keys.bind` and `keys.config`, called by scripts on the control thread.
pub fn dispatch(api: &str, input: &Value) -> Result<Value, String> {
    match api {
        "keys.bind" => {
            let keys = input["key"]
                .as_str()
                .ok_or_else(|| render("f025", &[("key", "key")]))?;
            let mut bound = BOUND.lock().map_err(|error| error.to_string())?;
            bind(&mut bound, get(), keys).map(Value::from)
        }
        "keys.config" => Ok(config(get())),
        other => Err(render("f027", &[("other", other)])),
    }
}

/// Adds `keys` to `bound` unless it is there already, and returns its name.
fn bind(bound: &mut Vec<Binding>, keybinds: &Keybinds, keys: &str) -> Result<String, String> {
    let binding = binding(keys).ok_or_else(|| render("keys_invalid", &[("keys", keys)]))?;
    if let Some(command) = keybinds.command_of(&binding) {
        return Err(render(
            "keys_shared",
            &[("keys", keys), ("command", command.name())],
        ));
    }
    let name = name(&binding);
    if !bound.iter().any(|other| self::name(other) == name) {
        bound.push(binding);
    }
    Ok(name)
}

/// One spelling for every way of writing a key: `Ctrl+Shift+P` for `Shift+Ctrl+P`.
fn name(binding: &Binding) -> String {
    let modifiers = binding.modifiers;
    [
        (modifiers.ctrl, "Ctrl+"),
        (modifiers.shift, "Shift+"),
        (modifiers.alt, "Alt+"),
    ]
    .into_iter()
    .filter_map(|(held, text)| held.then_some(text))
    .chain([binding.key.symbol_or_name()])
    .collect()
}

/// The keys of the commands as read, and `[scripts]`.
fn config(keybinds: &Keybinds) -> Value {
    let mut config: Map<String, Value> = Command::ALL
        .into_iter()
        .zip(&keybinds.bindings)
        .map(|(command, binding)| (command.name().into(), json!(binding.text)))
        .collect();
    let scripts = keybinds
        .scripts
        .iter()
        .map(|(binding, path)| (binding.text.clone(), json!(path)))
        .collect();
    config.insert(TABLE.into(), Value::Object(scripts));
    Value::Object(config)
}

/// Consumes the presses of bound keys and returns their names in order. A key held
/// down counts once.
pub fn take_script_presses(input: &mut egui::InputState) -> Vec<String> {
    match BOUND.lock() {
        Ok(bound) => presses(&bound, input),
        Err(_) => Vec::new(),
    }
}

fn presses(bound: &[Binding], input: &mut egui::InputState) -> Vec<String> {
    let mut names = Vec::new();
    let mut typed = Vec::new();
    input.events.retain(|event| {
        let Event::Key {
            key,
            physical_key,
            modifiers,
            pressed: true,
            repeat,
        } = event
        else {
            return true;
        };
        let exact = |binding: &&Binding| {
            binding_extra(binding, *key, *physical_key, *modifiers) == Some(Modifiers::NONE)
        };
        let Some(binding) = bound.iter().find(exact) else {
            return true;
        };
        if !repeat {
            names.push(name(binding));
        }
        typed.push(binding.key);
        false
    });
    for key in typed {
        drop_typed(input, key);
    }
    names
}

/// The run that calls the function bound to the key `name`.
pub fn script_request(name: &str) -> crate::execution::Request {
    let name = serde_json::to_string(name).expect("a string serializes");
    crate::execution::Request {
        scope: SCRIPT_SCOPE,
        chars: 0..0,
        code: format!("__replRunKey({name})"),
        first_line: 1,
    }
}

#[cfg(test)]
mod tests;
