//! The external editor registered in `status.json`, used to open `config.toml`.
//! The app never rewrites `config.toml`: it only creates a commented template
//! when the file is missing.
use std::path::{Path, PathBuf};

const KEY: &str = "external_editor";

pub fn status_path() -> Result<PathBuf, String> {
    Ok(crate::resources::user_directory()?.join("status.json"))
}

/// The registered editor. A missing file means none; a broken file is reported
/// and also means none.
pub fn read(path: &Path) -> (Option<PathBuf>, Option<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (None, None),
        Err(error) => return (None, Some(status_error(path, error))),
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(root)) => match root.get(KEY) {
            None | Some(serde_json::Value::Null) => (None, None),
            Some(serde_json::Value::String(editor)) if !editor.is_empty() => {
                (Some(PathBuf::from(editor)), None)
            }
            Some(other) => (None, Some(status_error(path, format!("{KEY}: {other}")))),
        },
        Ok(other) => (None, Some(status_error(path, other))),
        Err(error) => (None, Some(status_error(path, error))),
    }
}

/// Stores `input` as the editor; empty input removes the registration.
/// Anything other than the full path of an existing file is rejected unsaved.
pub fn save(path: &Path, input: &str) -> Result<Option<PathBuf>, String> {
    let input = input.trim();
    let editor = if input.is_empty() {
        None
    } else {
        let editor = PathBuf::from(input);
        if !editor.is_absolute() || !editor.is_file() {
            return Err(crate::i18n::t("settings_editor_invalid").into());
        }
        Some(editor)
    };
    let mut root = serde_json::Map::new();
    if let Some(editor) = &editor {
        root.insert(KEY.into(), editor.to_string_lossy().into());
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| status_error(path, e))?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|e| status_error(path, e))?;
    }
    std::fs::write(path, text).map_err(|e| status_error(path, e))?;
    Ok(editor)
}

/// Opens `config` in `editor`, first creating the commented template when
/// `config` does not exist.
pub fn open_config(editor: Option<&Path>, config: &Path) -> Result<(), String> {
    let editor = editor.ok_or_else(|| crate::i18n::t("settings_editor_missing").to_string())?;
    crate::resources::write_missing(config, crate::config::TEMPLATE.as_bytes())?;
    std::process::Command::new(editor)
        .arg(config)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            crate::i18n::render(
                "settings_launch_error",
                &[
                    ("editor", editor.display().to_string()),
                    ("error", error.to_string()),
                ],
            )
        })
}

fn status_error(path: &Path, error: impl std::fmt::Display) -> String {
    crate::i18n::render(
        "settings_status_error",
        &[("error", format!("{}: {error}", path.display()))],
    )
}

#[cfg(test)]
#[path = "external_editor_tests.rs"]
mod tests;
