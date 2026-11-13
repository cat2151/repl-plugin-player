//! `config.toml` in the user directory: window position and size, and the extension
//! DLLs to load (`[[extension]]`, read only at startup). The `[layout]` table
//! is the user's layout, read by `gui::dock::persist`. The app only
//! reads it; invalid items fall back to their defaults and are reported as
//! messages, and tables it does not know are ignored.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// `None` lets the OS place the window.
    pub position: Option<[f32; 2]>,
    pub size: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub window: Window,
    /// The `name` of each `[[extension]]`, in the file's order; none is loaded otherwise.
    pub extensions: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: Window {
                position: None,
                size: [1250.0, 800.0],
            },
            extensions: Vec::new(),
        }
    }
}

/// Written once, only when `config.toml` does not exist, so the user sees every key.
pub const TEMPLATE: &str = "\
# [window]
# x = 100
# y = 100
# width = 1250
# height = 800

# [layout.root]
# row = [{ width = 1, view = \"editor\" }, { width = 1, view = \"main\" }]

# [[layout.window]]
# column = [{ height = 1, view = \"builtin/keyboard\" }, { height = 1, view = \"scope\" }]
# [layout.window.monitor_placement]
# monitor = \"right\"
# left_percent = 0
# top_percent = 0
# width_percent = 100
# height_percent = 100

[[extension]]
name = \"builtin/keyboard\"
";

pub fn path() -> Result<PathBuf, String> {
    Ok(crate::resources::user_directory()?.join("config.toml"))
}

/// Reads `path`; a missing file is read as [`TEMPLATE`], as if it had been written.
pub fn read(path: &Path) -> (Config, Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => parse(TEMPLATE),
        Err(error) => (
            Config::default(),
            vec![crate::i18n::render(
                "config_read_error",
                &[("error", error.to_string())],
            )],
        ),
    }
}

pub fn parse(text: &str) -> (Config, Vec<String>) {
    let mut config = Config::default();
    let mut errors = Vec::new();
    let root: toml::Table = match toml::from_str(text) {
        Ok(root) => root,
        Err(error) => {
            errors.push(crate::i18n::render(
                "config_read_error",
                &[("error", error.to_string())],
            ));
            return (config, errors);
        }
    };
    let window = table(&root, "window", &mut errors);
    let x = number(&window, "window.x", "x", &mut errors);
    let y = number(&window, "window.y", "y", &mut errors);
    match (x, y) {
        (Some(x), Some(y)) => config.window.position = Some([x, y]),
        (None, None) => {}
        _ => errors.push(crate::i18n::t("config_position_pair").into()),
    }
    for (index, key) in ["width", "height"].into_iter().enumerate() {
        let name = format!("window.{key}");
        match number(&window, &name, key, &mut errors) {
            Some(value) if value > 0.0 => config.window.size[index] = value,
            Some(_) => errors.push(crate::i18n::render("config_positive", &[("key", name)])),
            None => {}
        }
    }
    config.extensions = extensions(&root, &mut errors);
    (config, errors)
}

/// The names of `[[extension]]`; an item without a string `name` of the form
/// `<dll>/<pane>`, or repeating one, is reported and skipped.
fn extensions(root: &toml::Table, errors: &mut Vec<String>) -> Vec<String> {
    let items = match root.get("extension") {
        None => return Vec::new(),
        Some(toml::Value::Array(items)) => items,
        Some(_) => {
            errors.push(crate::i18n::t("config_extension_array").into());
            return Vec::new();
        }
    };
    let mut names: Vec<String> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let position = (index + 1).to_string();
        match item.get("name") {
            Some(toml::Value::String(name)) if crate::extension::split(name).is_none() => errors
                .push(crate::i18n::render(
                    "config_extension_form",
                    &[("name", name.clone())],
                )),
            Some(toml::Value::String(name)) if names.contains(name) => errors.push(
                crate::i18n::render("config_extension_duplicate", &[("name", name.clone())]),
            ),
            Some(toml::Value::String(name)) => names.push(name.clone()),
            _ => errors.push(crate::i18n::render(
                "config_extension_name",
                &[("index", position)],
            )),
        }
    }
    names
}

/// The `[name]` table, or an empty one when absent or not a table.
fn table(root: &toml::Table, name: &str, errors: &mut Vec<String>) -> toml::Table {
    match root.get(name) {
        Some(toml::Value::Table(table)) => table.clone(),
        Some(_) => {
            errors.push(crate::i18n::render("config_table", &[("key", name.into())]));
            toml::Table::new()
        }
        None => toml::Table::new(),
    }
}

/// An integer or float value; anything else is reported and treated as absent.
fn number(table: &toml::Table, name: &str, key: &str, errors: &mut Vec<String>) -> Option<f32> {
    match table.get(key)? {
        toml::Value::Integer(value) => Some(*value as f32),
        toml::Value::Float(value) if value.is_finite() => Some(*value as f32),
        _ => {
            errors.push(crate::i18n::render(
                "config_number",
                &[("key", name.into())],
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn extensions_are_read_in_order() {
        let (config, errors) = parse(
            "[[extension]]\nname = \"builtin/keyboard\"\n[[extension]]\nname = \"other/probe\"\n",
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            config.extensions,
            names(&["builtin/keyboard", "other/probe"])
        );
    }

    #[test]
    fn without_extensions_none_is_loaded() {
        let (config, errors) = parse("[window]\nwidth = 900\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert!(config.extensions.is_empty());
    }

    #[test]
    fn repeated_extension_is_reported_and_skipped() {
        let (config, errors) =
            parse("[[extension]]\nname = \"builtin/keyboard\"\n[[extension]]\nname = \"builtin/keyboard\"\n");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("keyboard"), "{errors:?}");
        assert_eq!(config.extensions, names(&["builtin/keyboard"]));
    }

    #[test]
    fn extension_name_without_dll_and_pane_is_reported_and_skipped() {
        for name in ["keyboard", "/keyboard", "builtin/", "a/b/c"] {
            let text = format!("[[extension]]\nname = \"{name}\"\n");
            let (config, errors) = parse(&text);
            assert_eq!(errors.len(), 1, "{name} {errors:?}");
            assert!(errors[0].contains(name), "{errors:?}");
            assert!(config.extensions.is_empty(), "{name}");
        }
    }

    #[test]
    fn extension_without_string_name_is_reported_and_skipped() {
        let (config, errors) = parse(
            "[[extension]]\nname = 3\n[[extension]]\n[[extension]]\nname = \"builtin/keyboard\"\n",
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(config.extensions, names(&["builtin/keyboard"]));
        for text in [
            "extension = \"keyboard\"\n",
            "[extension]\nname = \"builtin/keyboard\"\n",
        ] {
            let (config, errors) = parse(text);
            assert_eq!(errors.len(), 1, "{text} {errors:?}");
            assert!(config.extensions.is_empty(), "{text}");
        }
    }

    #[test]
    fn empty_file_is_default() {
        assert_eq!(parse(""), (Config::default(), vec![]));
    }

    #[test]
    fn every_key_is_applied() {
        let (config, errors) =
            parse("[window]\nx = -1200\ny = 100.5\nwidth = 1600\nheight = 900\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config.window.position, Some([-1200.0, 100.5]));
        assert_eq!(config.window.size, [1600.0, 900.0]);
    }

    #[test]
    fn omitted_keys_keep_defaults() {
        let (config, errors) = parse("[window]\nwidth = 900\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config.window.position, None);
        assert_eq!(config.window.size, [900.0, 800.0]);
    }

    #[test]
    fn wrong_types_fall_back_per_item() {
        let (config, errors) = parse("window = 3\n");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(config.window, Config::default().window);
    }

    #[test]
    fn negative_values_fall_back_per_item() {
        let (config, errors) = parse("[window]\nwidth = -5\nheight = 700\n");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(config.window.size, [1250.0, 700.0]);
    }

    #[test]
    fn old_panes_table_is_ignored() {
        for text in [
            "[panes]\nleft_min_percent = 10\nright_min_percent = 10\n",
            "[panes]\nleft_min_percent = 60\nright_min_percent = 50\n",
            "[panes]\nleft_min_percent = \"20\"\nright_min_percent = -1\n",
            "panes = 3\n",
        ] {
            assert_eq!(parse(text), (Config::default(), vec![]), "{text}");
        }
    }

    #[test]
    fn half_a_position_is_reported() {
        let (config, errors) = parse("[window]\nx = 10\n");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(config.window.position, None);
    }

    #[test]
    fn broken_toml_is_reported() {
        let (config, errors) = parse("[window\n");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn template_is_the_default_configuration_with_keyboard() {
        let keyboard = Config {
            extensions: names(&["builtin/keyboard"]),
            ..Config::default()
        };
        assert_eq!(parse(TEMPLATE), (keyboard, vec![]));
        let uncommented = TEMPLATE.replace("# ", "");
        let (config, errors) = parse(&uncommented);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config.window.position, Some([100.0, 100.0]));
        assert_eq!(config.window.size, Config::default().window.size);
        assert_eq!(config.extensions, names(&["builtin/keyboard"]));
    }

    #[test]
    fn missing_file_is_the_template_without_errors() {
        let directory = std::env::temp_dir().join(format!("rpp-config-{}", std::process::id()));
        let (config, errors) = read(&directory.join("config.toml"));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config, parse(TEMPLATE).0);
        assert_eq!(config.extensions, names(&["builtin/keyboard"]));
    }

    #[test]
    fn written_template_loads_keyboard() {
        let directory =
            std::env::temp_dir().join(format!("rpp-config-template-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("config.toml");
        crate::resources::write_missing(&file, TEMPLATE.as_bytes()).unwrap();
        let result = read(&file);
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(result.1.is_empty(), "{:?}", result.1);
        assert_eq!(result.0.extensions, names(&["builtin/keyboard"]));
    }

    #[test]
    fn existing_file_is_read() {
        let directory =
            std::env::temp_dir().join(format!("rpp-config-read-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("config.toml");
        std::fs::write(&file, "[window]\nheight = 600\n").unwrap();
        let result = read(&file);
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(result.0.window.size, [1250.0, 600.0]);
        assert!(result.1.is_empty());
    }
}
