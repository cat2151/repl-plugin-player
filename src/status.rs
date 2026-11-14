//! `status.json` in the user directory: what the app's menus set, one key each. Writing
//! a key keeps the others.
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const LAYOUT_AUTO: &str = "layout_auto";
const EASY: &str = "easy";
const EXTENSIONS: &str = "extensions";
/// The extensions on while `status.json` has no `extensions`.
pub const DEFAULT_EXTENSIONS: [&str; 1] = ["builtin/keyboard"];

pub fn path() -> Result<PathBuf, String> {
    Ok(crate::resources::user_directory()?.join("status.json"))
}

/// The keys of the file; a missing file has none.
pub fn read(path: &Path) -> Result<Map<String, Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error_text(path, error)),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(root)) => Ok(root),
        Ok(other) => Err(error_text(path, other)),
        Err(error) => Err(error_text(path, error)),
    }
}

/// Sets `key` to `value` (`None` removes it). A file that does not read is not
/// overwritten.
pub fn write(path: &Path, key: &str, value: Option<Value>) -> Result<(), String> {
    let mut root = read(path)?;
    match value {
        Some(value) => root.insert(key.into(), value),
        None => root.remove(key),
    };
    let text = serde_json::to_string_pretty(&root).map_err(|e| error_text(path, e))?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|e| error_text(path, e))?;
    }
    std::fs::write(path, text).map_err(|e| error_text(path, e))
}

/// Whether the layout is the auto-saved one; `true` when unset or unreadable.
pub fn layout_auto(path: &Path) -> (bool, Option<String>) {
    flag(path, LAYOUT_AUTO)
}

pub fn save_layout_auto(path: &Path, auto: bool) -> Result<(), String> {
    write(path, LAYOUT_AUTO, Some(auto.into()))
}

/// Whether startup opens and runs `easy-init`; `true` when unset or unreadable.
pub fn easy(path: &Path) -> (bool, Option<String>) {
    flag(path, EASY)
}

pub fn save_easy(path: &Path, easy: bool) -> Result<(), String> {
    write(path, EASY, Some(easy.into()))
}

/// The `<dll>/<pane>` names of the extensions on, in order; [`DEFAULT_EXTENSIONS`] when
/// unset or unreadable. An item that is not such a name, or repeats one, is reported and
/// skipped.
pub fn extensions(path: &Path) -> (Vec<String>, Option<String>) {
    let default = || DEFAULT_EXTENSIONS.map(String::from).to_vec();
    let items = match read(path).map(|root| root.get(EXTENSIONS).cloned()) {
        Ok(None) => return (default(), None),
        Ok(Some(Value::Array(items))) => items,
        Ok(Some(other)) => {
            let error = error_text(path, format!("{EXTENSIONS}: {other}"));
            return (default(), Some(error));
        }
        Err(error) => return (default(), Some(error)),
    };
    let mut names: Vec<String> = Vec::new();
    let mut wrong = Vec::new();
    for item in items {
        match item {
            Value::String(name)
                if crate::extension::split(&name).is_some() && !names.contains(&name) =>
            {
                names.push(name)
            }
            other => wrong.push(other.to_string()),
        }
    }
    let error = (!wrong.is_empty())
        .then(|| error_text(path, format!("{EXTENSIONS}: {}", wrong.join(", "))));
    (names, error)
}

pub fn save_extensions(path: &Path, names: &[String]) -> Result<(), String> {
    write(path, EXTENSIONS, Some(names.into()))
}

/// A boolean key that is `true` when unset or unreadable.
fn flag(path: &Path, key: &str) -> (bool, Option<String>) {
    match read(path).map(|root| root.get(key).cloned()) {
        Ok(None) => (true, None),
        Ok(Some(Value::Bool(value))) => (value, None),
        Ok(Some(other)) => (true, Some(error_text(path, format!("{key}: {other}")))),
        Err(error) => (true, Some(error)),
    }
}

pub fn error_text(path: &Path, error: impl std::fmt::Display) -> String {
    crate::i18n::render(
        "settings_status_error",
        &[("error", format!("{}: {error}", path.display()))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_written_beside_the_others_and_read_back() {
        let directory = std::env::temp_dir().join(format!("rpp-status-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let path = directory.join("status.json");
        assert_eq!(layout_auto(&path), (true, None));
        assert_eq!(easy(&path), (true, None));
        write(&path, "external_editor", Some("C:\\editor.exe".into())).unwrap();
        save_layout_auto(&path, false).unwrap();
        save_easy(&path, false).unwrap();
        assert_eq!(layout_auto(&path), (false, None));
        assert_eq!(easy(&path), (false, None));
        let root = read(&path).unwrap();
        assert_eq!(root["external_editor"], "C:\\editor.exe");

        std::fs::write(&path, "{").unwrap();
        assert!(save_layout_auto(&path, true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{");
        let (auto, error) = layout_auto(&path);
        assert!(auto && error.is_some());
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn extensions_are_keyboard_until_saved_and_skip_wrong_items() {
        let directory =
            std::env::temp_dir().join(format!("rpp-status-extensions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let path = directory.join("status.json");
        assert_eq!(extensions(&path), (vec!["builtin/keyboard".into()], None));
        save_extensions(&path, &[]).unwrap();
        assert_eq!(extensions(&path), (vec![], None));
        let names = vec!["other/probe".to_string(), "builtin/keyboard".into()];
        save_extensions(&path, &names).unwrap();
        assert_eq!(extensions(&path), (names, None));

        let wrong = r#"{"extensions": ["a/b", 3, "keyboard", "a/b", "c/d"]}"#;
        std::fs::write(&path, wrong).unwrap();
        let (names, error) = extensions(&path);
        assert_eq!(names, ["a/b", "c/d"]);
        let error = error.unwrap();
        assert!(
            error.contains('3') && error.contains("\"keyboard\""),
            "{error}"
        );
        std::fs::write(&path, r#"{"extensions": "a/b"}"#).unwrap();
        let (names, error) = extensions(&path);
        assert_eq!(names, ["builtin/keyboard"]);
        assert!(error.is_some());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
