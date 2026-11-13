//! The external editor registered in `status.json`, used to open `config.toml`.
//! The app never rewrites `config.toml`: it only creates a commented template
//! when the file is missing.
use std::path::{Path, PathBuf};

const KEY: &str = "external_editor";

/// The registered editor. A missing file means none; a broken file is reported
/// and also means none.
pub fn read(path: &Path) -> (Option<PathBuf>, Option<String>) {
    match crate::status::read(path).map(|root| root.get(KEY).cloned()) {
        Ok(None | Some(serde_json::Value::Null)) => (None, None),
        Ok(Some(serde_json::Value::String(editor))) if !editor.is_empty() => {
            (Some(PathBuf::from(editor)), None)
        }
        Ok(Some(other)) => (None, Some(status_error(path, format!("{KEY}: {other}")))),
        Err(error) => (None, Some(error)),
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
    let value = editor.as_ref().map(|e| e.to_string_lossy().into());
    crate::status::write(path, KEY, value)?;
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
    crate::status::error_text(path, error)
}

#[cfg(test)]
#[path = "external_editor_tests.rs"]
mod tests;
