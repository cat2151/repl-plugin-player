//! `status.json` in the user directory: what the app's menus set, one key each. Writing
//! a key keeps the others.
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

const LAYOUT_AUTO: &str = "layout_auto";

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
    match read(path).map(|root| root.get(LAYOUT_AUTO).cloned()) {
        Ok(None) => (true, None),
        Ok(Some(Value::Bool(auto))) => (auto, None),
        Ok(Some(other)) => (
            true,
            Some(error_text(path, format!("{LAYOUT_AUTO}: {other}"))),
        ),
        Err(error) => (true, Some(error)),
    }
}

pub fn save_layout_auto(path: &Path, auto: bool) -> Result<(), String> {
    write(path, LAYOUT_AUTO, Some(auto.into()))
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
        write(&path, "external_editor", Some("C:\\editor.exe".into())).unwrap();
        save_layout_auto(&path, false).unwrap();
        assert_eq!(layout_auto(&path), (false, None));
        let root = read(&path).unwrap();
        assert_eq!(root["external_editor"], "C:\\editor.exe");

        std::fs::write(&path, "{").unwrap();
        assert!(save_layout_auto(&path, true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{");
        let (auto, error) = layout_auto(&path);
        assert!(auto && error.is_some());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
