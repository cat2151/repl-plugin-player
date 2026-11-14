use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

include!(concat!(env!("OUT_DIR"), "/builtin_snippets.rs"));
const STARTUP_SOURCE: &str = "builtin/try-play.js";

// Follow the trial workflow; unlisted built-ins and user snippets use name order.
const BUILTIN_ORDER: &[&str] = &[
    "try-play",
    "try-init-saw",
    "try-chords",
    "try-preset",
    "try-delay",
    "try-surge-ui",
    "try-effect-ui",
    "try-mml",
    "try-loop-mml",
    "try-loop-mml-multi-track",
    "try-instrument",
    "try-parameter-ui",
    "try-patch-browse",
    "try-cut-paste",
    "play",
    "stop-playback",
    "show-instrument-ui",
    "show-effect-ui",
    "set-track-volume",
    "close-parameter-ui",
    "add-midi-clip",
];

#[derive(Clone, Debug)]
pub struct Snippet {
    pub name: String,
    pub group: &'static str,
    pub code: String,
}

pub struct Library {
    pub builtin: PathBuf,
    pub user: Option<PathBuf>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            builtin: builtin_directory(
                &crate::resources::executable_directory()
                    .expect("Executable directory validated at startup"),
            ),
            user: std::env::var_os("LOCALAPPDATA")
                .map(|root| PathBuf::from(root).join("repl-plugin-player/snippets")),
        }
    }
}

impl Library {
    /// Resolve explicit namespaces using the same roots as the snippet browser.
    /// A present external catalog is authoritative, including missing-file errors.
    pub fn source(&self, name: &str) -> Result<String, String> {
        let resolve = || -> Result<String, String> {
            let (group, relative) = name
                .split_once('/')
                .ok_or("Expected builtin/... or user/...")?;
            if !relative.ends_with(".js") || relative.contains('\\') {
                return Err("Expected a relative .js path with forward slashes".into());
            }
            for part in relative.split('/') {
                valid_name(part)?;
            }
            let (root, builtin) = match group {
                "builtin" => (&self.builtin, true),
                "user" => (
                    self.user
                        .as_ref()
                        .ok_or("User snippet directory unavailable")?,
                    false,
                ),
                _ => return Err("Expected builtin/... or user/...".into()),
            };
            if builtin && !root.try_exists().map_err(|e| e.to_string())? {
                return EMBEDDED
                    .iter()
                    .find(|(path, _)| *path == relative)
                    .map(|(_, code)| localize_builtin(code))
                    .ok_or_else(|| "Embedded script not found".into());
            }
            let root = root.canonicalize().map_err(|e| e.to_string())?;
            let path = root
                .join(relative)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            if !path.starts_with(&root) {
                return Err("Script resolves outside its snippet directory".into());
            }
            let code = read(&path)?;
            Ok(if builtin {
                localize_builtin(&code)
            } else {
                code
            })
        };
        resolve().map_err(|error| format!("{name}: {error}"))
    }

    pub fn initial(&self) -> Result<String, String> {
        self.source(STARTUP_SOURCE)
    }

    pub fn load(&self) -> Result<Vec<Snippet>, String> {
        let mut result = if self
            .builtin
            .try_exists()
            .map_err(|error| error.to_string())?
        {
            directory(&self.builtin, "Built-in")?
        } else {
            EMBEDDED
                .iter()
                .filter(|(path, _)| !path.contains('/'))
                .map(|(name, code)| Snippet {
                    name: Path::new(name)
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .expect("Embedded JS path")
                        .into(),
                    group: "Built-in",
                    code: localize_builtin(code),
                })
                .collect()
        };
        if let Some(user) = &self.user {
            if user.try_exists().map_err(|error| error.to_string())? {
                result.extend(directory(user, "User")?);
            }
        }
        result.sort_by_key(|snippet| {
            let rank = if snippet.group == "Built-in" {
                BUILTIN_ORDER.iter().position(|name| *name == snippet.name)
            } else {
                None
            };
            (
                snippet.group,
                rank.unwrap_or(usize::MAX),
                snippet.name.to_lowercase(),
            )
        });
        Ok(result)
    }

    pub fn save(&self, name: &str, code: &str) -> Result<PathBuf, String> {
        let name = valid_name(name)?;
        if code.trim().is_empty() {
            return Err(crate::i18n::t("m071").into());
        }
        let directory = self.user.as_ref().ok_or(crate::i18n::t("m072"))?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let path = directory.join(format!("{name}.js"));
        // Never replace an existing snippet. The user can edit its .js file.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if let Err(error) = file
            .write_all(code.as_bytes())
            .and_then(|_| file.sync_all())
        {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(format!("{}: {error}", path.display()));
        }
        Ok(path)
    }
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn directory(path: &Path, group: &'static str) -> Result<Vec<Snippet>, String> {
    let mut result = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
            || !entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
        {
            continue;
        }
        let file = entry.path();
        let name = file
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or(crate::i18n::t("m073"))?;
        result.push(Snippet {
            name: name.into(),
            group,
            code: if group == "Built-in" {
                localize_builtin(&read(&file)?)
            } else {
                read(&file)?
            },
        });
    }
    Ok(result)
}

fn valid_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if name.is_empty()
        || name.chars().count() > 100
        || name.ends_with('.')
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        || reserved.contains(&base.as_str())
    {
        return Err(crate::i18n::t("m074").into());
    }
    Ok(name)
}

#[cfg(test)]
#[path = "snippets_tests.rs"]
mod tests;

pub(crate) fn localize_builtin(code: &str) -> String {
    code.split_inclusive('\n')
        .map(|line| {
            let content = line.trim_start_matches([' ', '\t']);
            let indent = &line[..line.len() - content.len()];
            for prefix in ["// @text ", "// %% @text "] {
                if let Some(key) = content.trim_end_matches(['\r', '\n']).strip_prefix(prefix) {
                    return format!(
                        "{indent}{}{}{}",
                        if prefix.contains("%%") {
                            "// %% "
                        } else {
                            "// "
                        },
                        crate::i18n::t(key),
                        if line.ends_with('\n') { "\n" } else { "" }
                    );
                }
            }
            line.to_owned()
        })
        .collect()
}

fn builtin_directory(directory: &Path) -> PathBuf {
    crate::resources::project_root(directory)
        .unwrap_or_else(|| directory.to_owned())
        .join("snippets/builtin")
}
