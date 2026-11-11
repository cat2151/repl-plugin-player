//! Resolve development resources relative to the exe and deploy embedded resources.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const BRIDGE: &[u8] = include_bytes!("../build/out/repl_bridge.dll");
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

/// Only the repository's exact target/release directory uses source catalogs.
pub fn release_root(start: &Path) -> Option<PathBuf> {
    let root = start.parent()?.parent()?;
    (start == root.join("target/release") && project_root(root).as_deref() == Some(root))
        .then(|| root.to_owned())
}

pub fn user_directory() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|root| PathBuf::from(root).join("repl-plugin-player"))
        .ok_or_else(|| "LOCALAPPDATA is unavailable".into())
}

/// Publish complete files without overwriting an existing file, including a racing writer.
pub fn write_missing(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let publish = || -> std::io::Result<()> {
        if path.try_exists()? {
            return Ok(());
        }
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
            match std::fs::hard_link(&temporary, path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(error) => Err(error),
            }
        })();
        let _ = std::fs::remove_file(&temporary);
        result
    };
    publish().map_err(|e| format!("{}: {e}", path.display()))
}

pub fn bridge() -> Result<PathBuf, String> {
    let start = executable_directory()?;
    if let Some(root) = project_root(&start) {
        return Ok(root.join("build/out/repl_bridge.dll"));
    }
    let mut hash = DefaultHasher::new();
    BRIDGE.hash(&mut hash);
    let path = user_directory()?
        .join("native")
        .join(format!("{:016x}", hash.finish()))
        .join("repl_bridge.dll");
    write_missing(&path, BRIDGE)?;
    let actual = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if actual != BRIDGE {
        return Err(format!(
            "{}: cached bridge content mismatch",
            path.display()
        ));
    }
    Ok(path)
}
