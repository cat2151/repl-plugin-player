//! A state file that restores one of cat-plugin-player's catalog patches, found
//! from the patch's display path in that player's table of patch folders. A
//! patch whose state is its file converted, or is built on a kept template, is
//! made here; the rest are made by the player.
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

mod built;
pub mod clap;
mod dexed;
mod floe;
mod into;
mod roots;
mod surge_fx;

#[cfg(test)]
use roots::locate;
use roots::reference;

pub use into::{patch_into, StateOut};

/// Catalog plugins whose state the player makes with a temporary instance of
/// the plugin; the state of the rest is their patch file converted, or built
/// on a template ([`built`]).
const NEEDS_INSTANCE: [&str; 3] = [
    cmrt_server_config::DEXED_PLUGIN_ID,
    cmrt_server_config::FLOE_PLUGIN_ID,
    built::SFORZANDO_PLUGIN_ID,
];

/// Starts asking the player for its table on another thread, so the first
/// lookup waits for that answer instead of asking itself. A missing player is
/// left for that lookup to report.
pub fn preload() {
    std::thread::spawn(|| {
        let Ok(executable) = std::env::current_exe() else {
            return;
        };
        let Ok(player) = crate::patch_browser::resolve(&executable, std::env::var_os("PATH"))
        else {
            return;
        };
        let mut slot = TABLE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_none() {
            *slot = fetch(&player).ok();
        }
    });
}

pub fn file(format: &str, plugin_id: &str, patch: &str) -> Result<PathBuf, String> {
    state_file(&patch_reference(format, plugin_id, patch)?)
}

/// The player's reference to the catalog patch `patch` (its display path).
pub fn patch_reference(
    format: &str,
    plugin_id: &str,
    patch: &str,
) -> Result<PatchReference, String> {
    lookup(&crate::patch_browser::player()?, format, plugin_id, patch)
}

/// A state file for `reference`, made without an instance of the plugin.
pub fn state_file(reference: &PatchReference) -> Result<PathBuf, String> {
    let template = built::kept(&reference.plugin.id);
    write(
        &state_name(),
        &state(reference, template.as_deref(), ask_player)?,
    )
}

/// Fails on a patch [`into_instance`] could not read, before an instance exists.
pub fn check(reference: &PatchReference) -> Result<(), String> {
    into::check(reference)
}

/// The patch put into the track instance `instance`: `None` when it went in
/// directly (finishing after this returns), else a state file the caller loads
/// into the instance. `fresh` tells the instance is new.
pub fn into_instance(
    reference: PatchReference,
    instance: i32,
    fresh: bool,
) -> Result<Option<PathBuf>, String> {
    let prepared = into::prepare(reference, ask_player)?;
    let state = into::into(|| clap::Plugin::of(instance), prepared, fresh)?;
    state.map(|state| write(&state_name(), &state)).transpose()
}

/// Whether the state for `reference` is built on a template.
fn templated(reference: &PatchReference) -> bool {
    reference.plugin.format == "CLAP" && built::builds(&reference.plugin.id, &reference.source)
}

/// A file holding the catalog patch `patch` prepared for an evolve host,
/// which hands it to [`patch_into`] once the instance exists. Runs on any
/// thread; asks the player for a state only it makes.
pub fn prepared_file(format: &str, plugin_id: &str, patch: &str) -> Result<PathBuf, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let prepared = into::prepare(patch_reference(format, plugin_id, patch)?, ask_player)?;
    let name = format!(
        "patch-job-{}-{}.bin",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    write(&name, &prepared.to_bytes()?)
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
    state_file(&effect_reference(
        &player, format, plugin_id, preset, value,
    )?)
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

/// This process's state file.
fn state_name() -> String {
    format!("patch-state-{}.bin", std::process::id())
}

/// The player's state for the serialized reference `input`.
fn ask_player(input: &[u8]) -> Result<Vec<u8>, String> {
    ask(&crate::patch_browser::player()?, "--restore-state", input)
}

fn write(name: &str, state: &[u8]) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join("repl-plugin-player").join(name);
    std::fs::create_dir_all(path.parent().expect("joined")).map_err(|e| e.to_string())?;
    std::fs::write(&path, state).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The state for `reference`, made here ([`made_here`]) or else the player's
/// answer to `restore`.
fn state(
    reference: &PatchReference,
    template: Option<&[u8]>,
    restore: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    made_here(reference, template)
        .unwrap_or_else(|| restore(&serde_json::to_vec(reference).map_err(|e| e.to_string())?))
}

/// The state for `reference` when it is made here: built on the plugin's
/// `template`, a Surge XT Effects state prepared ahead, or its patch file
/// converted when that is all it takes. `None` leaves it to the player.
fn made_here(
    reference: &PatchReference,
    template: Option<&[u8]>,
) -> Option<Result<Vec<u8>, String>> {
    let id = reference.plugin.id.as_str();
    if let (true, Some(template)) = (templated(reference), template) {
        return Some(built::build(template, id, &reference.source));
    }
    if let Source::Effect { path, .. } = &reference.source {
        if reference.plugin.format == "CLAP" && id == surge_fx::PLUGIN_ID {
            match surge_fx::state(path) {
                Ok(None) => {}
                made => return made.transpose(),
            }
        }
    }
    if let Source::Catalog { path, .. } = &reference.source {
        if reference.plugin.format == "CLAP"
            && cmrt_patches::supports_catalog_clap_plugin(id)
            && !NEEDS_INSTANCE.contains(&id)
        {
            let bundle = Path::new(&reference.plugin.bundle_path);
            return Some(
                cmrt_patches::prepare_catalog_clap_patch_state(id, bundle, path)
                    .map_err(|e| e.to_string()),
            );
        }
    }
    None
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

#[cfg(test)]
#[path = "patch_state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "patch_state_into_tests.rs"]
mod into_tests;
