//! `config.toml` in the user directory: window position/size and the minimum
//! width of each pane. The app only reads it; invalid items fall back to their
//! defaults and are reported as messages.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// `None` lets the OS place the window.
    pub position: Option<[f32; 2]>,
    pub size: [f32; 2],
}

/// Minimum pane widths as fractions of the window width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panes {
    pub left_min: f32,
    pub right_min: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    pub window: Window,
    pub panes: Panes,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: Window {
                position: None,
                size: [1250.0, 800.0],
            },
            panes: Panes::default(),
        }
    }
}

impl Default for Panes {
    fn default() -> Self {
        Self {
            left_min: 0.1,
            right_min: 0.1,
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
# [panes]
# left_min_percent = 10
# right_min_percent = 10
";

pub fn path() -> Result<PathBuf, String> {
    Ok(crate::resources::user_directory()?.join("config.toml"))
}

/// Reads `path`; a missing file is the default configuration without errors.
pub fn read(path: &Path) -> (Config, Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Config::default(), vec![]),
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
    let panes = table(&root, "panes", &mut errors);
    let mut minimum = |key: &str, slot: &mut f32| {
        let name = format!("panes.{key}");
        match number(&panes, &name, key, &mut errors) {
            Some(value) if (0.0..=100.0).contains(&value) => *slot = value / 100.0,
            Some(_) => errors.push(crate::i18n::render("config_percent", &[("key", name)])),
            None => {}
        }
    };
    minimum("left_min_percent", &mut config.panes.left_min);
    minimum("right_min_percent", &mut config.panes.right_min);
    if config.panes.left_min + config.panes.right_min > 1.0 {
        config.panes = Panes::default();
        errors.push(crate::i18n::t("config_percent_sum").into());
    }
    (config, errors)
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

    #[test]
    fn empty_file_is_default() {
        assert_eq!(parse(""), (Config::default(), vec![]));
    }

    #[test]
    fn every_key_is_applied() {
        let (config, errors) = parse(
            "[window]\nx = -1200\ny = 100.5\nwidth = 1600\nheight = 900\n\
             [panes]\nleft_min_percent = 20\nright_min_percent = 35.5\n",
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config.window.position, Some([-1200.0, 100.5]));
        assert_eq!(config.window.size, [1600.0, 900.0]);
        assert_eq!(config.panes.left_min, 0.2);
        assert_eq!(config.panes.right_min, 0.355);
    }

    #[test]
    fn omitted_keys_keep_defaults() {
        let (config, errors) = parse("[window]\nwidth = 900\n");
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config.window.position, None);
        assert_eq!(config.window.size, [900.0, 800.0]);
        assert_eq!(config.panes, Panes::default());
    }

    #[test]
    fn wrong_types_fall_back_per_item() {
        let (config, errors) =
            parse("window = 3\n[panes]\nleft_min_percent = \"20\"\nright_min_percent = 30\n");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(config.window, Config::default().window);
        assert_eq!(config.panes.left_min, 0.1);
        assert_eq!(config.panes.right_min, 0.3);
    }

    #[test]
    fn negative_values_fall_back_per_item() {
        let (config, errors) =
            parse("[window]\nwidth = -5\nheight = 700\n[panes]\nleft_min_percent = -1\n");
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(config.window.size, [1250.0, 700.0]);
        assert_eq!(config.panes, Panes::default());
    }

    #[test]
    fn minimums_over_one_hundred_fall_back_together() {
        let (config, errors) = parse("[panes]\nleft_min_percent = 60\nright_min_percent = 50\n");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(config.panes, Panes::default());
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
    fn template_is_the_default_configuration() {
        assert_eq!(parse(TEMPLATE), (Config::default(), vec![]));
        let uncommented = TEMPLATE.replace("# ", "");
        let (config, errors) = parse(&uncommented);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(config.window.position, Some([100.0, 100.0]));
        assert_eq!(config.window.size, Config::default().window.size);
        assert_eq!(config.panes, Panes::default());
    }

    #[test]
    fn missing_file_is_default_without_errors() {
        let directory = std::env::temp_dir().join(format!("rpp-config-{}", std::process::id()));
        assert_eq!(
            read(&directory.join("config.toml")),
            (Config::default(), vec![])
        );
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
