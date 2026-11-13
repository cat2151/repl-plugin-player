//! Resolve development resources relative to the exe and deploy embedded resources.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
};

const BRIDGE: &[u8] = include_bytes!("../build/out/repl_bridge.dll");
include!(concat!(env!("OUT_DIR"), "/keyboard_dll.rs"));
static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

pub fn executable_directory() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| "Cannot find executable directory".into())
}

pub fn project_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|root| {
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
        let value: toml::Value = toml::from_str(&manifest).ok()?;
        (value.get("package")?.get("name")?.as_str()? == "repl-plugin-player")
            .then(|| root.to_owned())
    })
}

/// The repository that built this exe; its `.js` and `.toml` files replace the embedded copies.
pub fn source_root() -> Option<PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| project_root(&executable_directory().ok()?))
        .clone()
}

/// A text resource read from [`source_root`] when present, otherwise the embedded copy.
pub struct Source {
    relative: &'static str,
    embedded: &'static str,
    text: OnceLock<Result<String, String>>,
}

impl Source {
    pub const fn new(relative: &'static str, embedded: &'static str) -> Self {
        Self {
            relative,
            embedded,
            text: OnceLock::new(),
        }
    }

    /// A present source tree is authoritative, so a missing file is an error.
    pub fn text(&self) -> Result<&str, String> {
        self.text
            .get_or_init(|| match source_root() {
                Some(root) => {
                    let path = root.join(self.relative);
                    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
                }
                None => Ok(self.embedded.to_owned()),
            })
            .as_deref()
            .map_err(Clone::clone)
    }
}

pub fn user_directory() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|root| PathBuf::from(root).join("repl-plugin-player"))
        .ok_or_else(|| "LOCALAPPDATA is unavailable".into())
}

/// Write `bytes` to a unique temporary file beside `path`, then hand it to `publish`.
fn publish_complete(
    path: &Path,
    bytes: &[u8],
    publish: impl FnOnce(&Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let parent = path.parent().expect("Resource has parent");
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".resource-{}-{}.tmp",
        std::process::id(),
        TEMPORARY_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        publish(&temporary)
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

/// Publish complete files without overwriting an existing file, including a racing writer.
pub fn write_missing(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let publish = || -> std::io::Result<()> {
        if path.try_exists()? {
            return Ok(());
        }
        publish_complete(path, bytes, |temporary| {
            match std::fs::hard_link(temporary, path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(error) => Err(error),
            }
        })
    };
    publish().map_err(|e| format!("{}: {e}", path.display()))
}

/// Replace `path` with complete `bytes`, so readers never see a partial file.
/// A racing writer of the same bytes may hold the target; its result is accepted.
pub fn write_replacing(path: &Path, bytes: &[u8]) -> Result<(), String> {
    publish_complete(path, bytes, |temporary| {
        std::fs::rename(temporary, path).or_else(|error| match std::fs::read(path) {
            Ok(actual) if actual == bytes => Ok(()),
            _ => Err(error),
        })
    })
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// The embedded DLL `name`, written once under the user directory in a folder named by
/// its content hash, so exes embedding different builds never share a file.
fn deploy(name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    let path = user_directory()?
        .join("native")
        .join(format!("{:016x}", hash.finish()))
        .join(name);
    write_missing(&path, bytes)?;
    let actual = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if actual != bytes {
        return Err(format!(
            "{}: cached {name} content mismatch",
            path.display()
        ));
    }
    Ok(path)
}

pub fn bridge() -> Result<PathBuf, String> {
    match source_root() {
        Some(root) => Ok(root.join("build/out/repl_bridge.dll")),
        None => deploy("repl_bridge.dll", BRIDGE),
    }
}

/// In the repository, `repl_keyboard.dll` from the Cargo profile directory this exe was
/// built into (test executables sit one level below it, in `deps`); elsewhere, the
/// embedded copy.
pub fn keyboard() -> Result<PathBuf, String> {
    if source_root().is_none() {
        if KEYBOARD.is_empty() {
            return Err("repl_keyboard.dll is not embedded in this exe".into());
        }
        return deploy("repl_keyboard.dll", KEYBOARD);
    }
    let directory = executable_directory()?;
    let profile = match directory.file_name() {
        Some(name) if name == "deps" => directory.parent().unwrap_or(&directory),
        _ => &directory,
    };
    Ok(profile.join("repl_keyboard.dll"))
}
