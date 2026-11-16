//! A state file that restores one of cat-plugin-player's catalog patches, made
//! by that player from the patch's display path.
use crate::{
    host::PluginKey,
    i18n::render,
    patch_reference::{PatchReference, Source},
};
use serde::Deserialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

/// The player's `--patch-roots` answer: where each plugin keeps its patches.
#[derive(Clone, Deserialize)]
struct Table {
    plugins: Vec<Plugin>,
}

#[derive(Clone, Deserialize)]
struct Plugin {
    format: String,
    id: String,
    bundle_path: String,
    roots: Vec<String>,
}

/// Kept for the process; asked again only when a lookup misses.
static TABLE: Mutex<Option<Table>> = Mutex::new(None);

pub fn file(format: &str, plugin_id: &str, patch: &str) -> Result<PathBuf, String> {
    let player = crate::patch_browser::player()?;
    let reference = lookup(&player, format, plugin_id, patch)?;
    restore(&player, &reference)
}

pub fn background_file(format: &str, plugin_id: &str, patch: &str) -> Result<PathBuf, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let player = crate::patch_browser::player()?;
    let reference = lookup(&player, format, plugin_id, patch)?;
    let name = format!(
        "patch-job-{}-{}.bin",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    restore_named(&player, &reference, &name)
}

/// The effect preset file `path` as a cell names it: relative to the one root
/// of the plugin holding it, its parts joined by `/`. A plugin whose presets
/// are built in names its root itself, which is empty.
pub fn effect_preset(format: &str, plugin_id: &str, path: &Path) -> Result<String, String> {
    let player = crate::patch_browser::player()?;
    let plugin = plugin(&player, format, plugin_id)?;
    let named: Vec<String> = plugin
        .roots
        .iter()
        .filter_map(|root| path.strip_prefix(root).ok())
        .map(|relative| {
            let parts: Vec<_> = relative.iter().map(|p| p.to_string_lossy()).collect();
            parts.join("/")
        })
        .collect();
    let display = path.display().to_string();
    match named.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(render(
            "patch_state_00",
            &[("patch", display), ("plugin", plugin_id.into())],
        )),
        _ => Err(render(
            "patch_state_01",
            &[("patch", display), ("plugin", plugin_id.into())],
        )),
    }
}

/// A state file restoring the effect preset `preset` (as [`effect_preset`]
/// names it) with the player's selector `value` inside the file.
pub fn effect_file(
    format: &str,
    plugin_id: &str,
    preset: &str,
    value: &str,
) -> Result<PathBuf, String> {
    let player = crate::patch_browser::player()?;
    let reference = effect_reference(&player, format, plugin_id, preset, value)?;
    restore(&player, &reference)
}

/// The player's reference to the effect preset, with the file under the one
/// root holding it.
pub fn effect_reference(
    player: &Path,
    format: &str,
    plugin_id: &str,
    preset: &str,
    value: &str,
) -> Result<PatchReference, String> {
    let plugin = plugin(player, format, plugin_id)?;
    let missing = |key| {
        render(
            key,
            &[("patch", preset.into()), ("plugin", plugin_id.into())],
        )
    };
    let parts: Vec<&str> = preset.split('/').filter(|_| !preset.is_empty()).collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains(['\\', ':']))
    {
        return Err(missing("patch_state_00"));
    }
    let found: Vec<PathBuf> = plugin
        .roots
        .iter()
        .map(|root| parts.iter().fold(PathBuf::from(root), |p, s| p.join(s)))
        .filter(|path| path.is_file())
        .collect();
    let path = match found.as_slice() {
        [one] => one.clone(),
        [] => return Err(missing("patch_state_00")),
        _ => return Err(missing("patch_state_01")),
    };
    Ok(PatchReference {
        plugin: PluginKey {
            format: format.into(),
            id: plugin_id.into(),
            name: String::new(),
            vendor: String::new(),
            bundle_path: plugin.bundle_path,
        },
        source: Source::Effect {
            path,
            value: value.into(),
        },
        display: preset.into(),
    })
}

/// The player's state for `reference`, written to this process's state file.
fn restore(player: &Path, reference: &PatchReference) -> Result<PathBuf, String> {
    restore_named(
        player,
        reference,
        &format!("patch-state-{}.bin", std::process::id()),
    )
}

fn restore_named(player: &Path, reference: &PatchReference, name: &str) -> Result<PathBuf, String> {
    let input = serde_json::to_vec(reference).map_err(|e| e.to_string())?;
    let state = ask(player, "--restore-state", &input)?;
    let path = std::env::temp_dir().join("repl-plugin-player").join(name);
    std::fs::create_dir_all(path.parent().expect("joined")).map_err(|e| e.to_string())?;
    std::fs::write(&path, state).map_err(|e| e.to_string())?;
    Ok(path)
}

fn lookup(
    player: &Path,
    format: &str,
    plugin_id: &str,
    patch: &str,
) -> Result<PatchReference, String> {
    let mut slot = TABLE.lock().map_err(|e| e.to_string())?;
    let cached = slot.is_some();
    if !cached {
        *slot = Some(fetch(player)?);
    }
    match reference(slot.as_ref().expect("filled"), format, plugin_id, patch) {
        Err(_) if cached => {
            *slot = Some(fetch(player)?);
            reference(slot.as_ref().expect("filled"), format, plugin_id, patch)
        }
        result => result,
    }
}

/// The roots of one plugin, asking the player again only when it is missing.
fn plugin(player: &Path, format: &str, plugin_id: &str) -> Result<Plugin, String> {
    let mut slot = TABLE.lock().map_err(|e| e.to_string())?;
    let find = |table: &Table| {
        table
            .plugins
            .iter()
            .find(|p| p.format == format && p.id == plugin_id)
            .cloned()
    };
    if let Some(found) = slot.as_ref().and_then(find) {
        return Ok(found);
    }
    let table = fetch(player)?;
    let found = find(&table);
    *slot = Some(table);
    found.ok_or_else(|| render("patch_state_02", &[("plugin", plugin_id.into())]))
}

fn fetch(player: &Path) -> Result<Table, String> {
    serde_json::from_slice(&ask(player, "--patch-roots", &[])?).map_err(|e| e.to_string())
}

/// Runs the player without a window for one answer on its standard streams.
fn ask(player: &Path, flag: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let failed =
        |error: String| render("patch_state_03", &[("flag", flag.into()), ("error", error)]);
    let mut command = Command::new(player);
    crate::surge_home::restore_for_child(&mut command);
    let mut child = command
        .arg(flag)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| failed(e.to_string()))?;
    let sent = child.stdin.take().expect("piped").write_all(input);
    let output = child
        .wait_with_output()
        .map_err(|e| failed(e.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(failed(format!("{} {}", output.status, stderr.trim())));
    }
    sent.map_err(|e| failed(e.to_string()))?;
    if output.stdout.is_empty() {
        return Err(failed(String::new()));
    }
    Ok(output.stdout)
}

fn reference(
    table: &Table,
    format: &str,
    plugin_id: &str,
    patch: &str,
) -> Result<PatchReference, String> {
    let plugin = table
        .plugins
        .iter()
        .find(|p| p.format == format && p.id == plugin_id)
        .ok_or_else(|| render("patch_state_02", &[("plugin", plugin_id.into())]))?;
    let (path, file) = locate(&plugin.roots, patch).map_err(|key| {
        render(
            key,
            &[("patch", patch.into()), ("plugin", plugin_id.into())],
        )
    })?;
    Ok(PatchReference {
        plugin: PluginKey {
            format: format.into(),
            id: plugin_id.into(),
            name: String::new(),
            vendor: String::new(),
            bundle_path: plugin.bundle_path.clone(),
        },
        source: Source::Catalog { path, file },
        display: patch.into(),
    })
}

/// The one place `display` names under `roots`: a root joined with it, or, when
/// its first part is the root's own name, the root's parent joined with it. The
/// file is the nearest existing ancestor; the rest is the program inside it.
/// The error is the message key.
fn locate(roots: &[String], display: &str) -> Result<(PathBuf, PathBuf), &'static str> {
    let parts: Vec<&str> = display.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains(['\\', ':']))
    {
        return Err("patch_state_00");
    }
    let mut found: Vec<(PathBuf, PathBuf)> = Vec::new();
    for root in roots.iter().map(Path::new) {
        let own_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(parts[0]));
        let bases = std::iter::once(Some(root)).chain(own_name.then(|| root.parent()));
        for base in bases.flatten() {
            let path: PathBuf = parts.iter().fold(base.to_path_buf(), |p, s| p.join(s));
            if let Some(file) = containing_file(base, &path) {
                if !found.iter().any(|(p, _)| *p == path) {
                    found.push((path, file));
                }
            }
        }
    }
    match found.len() {
        0 => Err("patch_state_00"),
        1 => Ok(found.remove(0)),
        _ => Err("patch_state_01"),
    }
}

fn containing_file(base: &Path, path: &Path) -> Option<PathBuf> {
    let file = path
        .ancestors()
        .take_while(|a| *a != base)
        .find(|a| a.exists())?;
    file.is_file().then(|| file.to_path_buf())
}

#[cfg(test)]
#[path = "patch_state_tests.rs"]
mod tests;
