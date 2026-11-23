//! Read the same cache and exclusive Role / Kind classifier as cat-plugin-player.
//!
//! `list` runs synchronously on the UI/control thread, so the catalog is read and classified once
//! and held for the whole process.
use cmrt_patches::{PatchRole, PatchRoleIndex, PatchRoleInput};
use cmrt_tui_core::patch_catalog_cache;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex, PoisonError},
};

type Groups = BTreeMap<(String, String), Vec<CatalogPatch>>;

struct CatalogPatch {
    display: String,
    category: Option<String>,
    drum_kit: bool,
}

struct Entry {
    id: String,
    plugin: String,
    patch: String,
    role: Option<PatchRole>,
    kind: Option<&'static str>,
    drum_kit: bool,
}

type Held = Mutex<Option<Arc<Vec<Entry>>>>;

static HELD: Held = Mutex::new(None);

struct Sources {
    cache: Option<PathBuf>,
    fallback: PathBuf,
    presets: PathBuf,
}

impl Sources {
    fn current() -> Result<Self, String> {
        let player = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or("LOCALAPPDATA is unavailable")?
            .join("cat-plugin-player");
        Ok(Self {
            cache: patch_catalog_cache::cache_file_path(),
            fallback: player.join("patch-catalog/surge-xt.json"),
            presets: player.join("patch-filter-presets.json"),
        })
    }

    fn read(&self) -> Result<Vec<Entry>, String> {
        let loaded = match self.cache.as_deref().filter(|p| p.exists()) {
            Some(_) => patch_catalog_cache::load(),
            None => patch_catalog_cache::load_from(&self.fallback),
        }
        .map_err(|e| format!("{e:#}"))?;
        let users: Vec<(String, String)> = match std::fs::read(&self.presets) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.to_string()),
        };
        let (snapshot, _) = loaded.into_parts();
        let mut groups = Groups::new();
        for patch in snapshot.audio_patches() {
            let plugin = snapshot
                .patch_plugins()
                .audio_info_for_ref(&patch.reference)
                .map_err(|e| e.to_string())?;
            let Some(id) = plugin.plugin_id.as_deref() else {
                continue;
            };
            if !cmrt_patches::supports_catalog_clap_plugin(id)
                || !Path::new(&plugin.plugin_path)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("clap"))
            {
                continue;
            }
            let display = &patch.reference.display;
            groups
                .entry((id.to_owned(), plugin.name.clone()))
                .or_default()
                .push(CatalogPatch {
                    display: display.clone(),
                    category: patch.selector_category.clone(),
                    drum_kit: snapshot
                        .load_measurements()
                        .get(display)
                        .is_some_and(|m| m.drum_kit),
                });
        }
        Ok(classify(groups, &users))
    }
}

pub fn list(role: &str, kind: &str) -> Result<Value, String> {
    let role = PatchRole::ALL
        .into_iter()
        .find(|r| r.key() == role)
        .ok_or_else(|| format!("Unknown catalog role: {role}"))?;
    let entries = held(&HELD, read_current)?;
    Ok(filter(&entries, role, kind))
}

/// Pitched patch candidates by plugin, from the same held catalog as `list`.
pub fn instruments() -> Result<Value, String> {
    let entries = held(&HELD, read_current)?;
    let mut plugins = BTreeMap::<_, (&str, Vec<&str>)>::new();
    for entry in entries
        .iter()
        .filter(|e| !e.drum_kit && e.role != Some(PatchRole::Drum))
    {
        plugins
            .entry(&entry.id)
            .or_insert_with(|| (&entry.plugin, Vec::new()))
            .1
            .push(&entry.patch);
    }
    Ok(plugins
        .into_iter()
        .map(|(id, (plugin, patches))| json!({"format":"CLAP", "pluginId":id, "plugin":plugin, "patches":patches}))
        .collect())
}

fn read_current() -> Result<Vec<Entry>, String> {
    Sources::current()?.read()
}

/// Builds the held catalog on another thread. A `list` that arrives meanwhile waits on the same
/// lock instead of reading again; a failure is left for `list` to report.
pub fn preload() {
    std::thread::spawn(|| {
        let _ = held(&HELD, read_current);
    });
}

/// Drops the held catalog and reads it again on another thread. Returns once that thread holds
/// the lock, so the next `list` waits for the new catalog instead of reading it itself; a failure
/// is left for `list` to report.
pub fn reload() {
    let (locked, wait) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = rebuild(
            &HELD,
            || {
                let _ = locked.send(());
            },
            read_current,
        );
    });
    let _ = wait.recv();
}

/// A failed read leaves the slot empty so the next call reads again.
fn held(
    slot: &Held,
    read: impl FnOnce() -> Result<Vec<Entry>, String>,
) -> Result<Arc<Vec<Entry>>, String> {
    let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(entries) = slot.as_ref() {
        return Ok(entries.clone());
    }
    fill(&mut slot, read)
}

/// Empties the slot and reads into it, calling `locked` once the lock is held.
fn rebuild(
    slot: &Held,
    locked: impl FnOnce(),
    read: impl FnOnce() -> Result<Vec<Entry>, String>,
) -> Result<Arc<Vec<Entry>>, String> {
    let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
    *slot = None;
    locked();
    fill(&mut slot, read)
}

fn fill(
    slot: &mut Option<Arc<Vec<Entry>>>,
    read: impl FnOnce() -> Result<Vec<Entry>, String>,
) -> Result<Arc<Vec<Entry>>, String> {
    let entries = Arc::new(read()?);
    *slot = Some(entries.clone());
    Ok(entries)
}

/// Roles are exclusive within one plugin, so each plugin's patches are classified together.
fn classify(groups: Groups, users: &[(String, String)]) -> Vec<Entry> {
    let mut entries = Vec::new();
    for ((id, name), patches) in groups {
        let normalized: Vec<_> = patches
            .iter()
            .map(|patch| patch.display.to_lowercase())
            .collect();
        let index = PatchRoleIndex::build(
            patches
                .iter()
                .zip(&normalized)
                .map(|(patch, n)| PatchRoleInput {
                    display: &patch.display,
                    normalized_display: n,
                    selector_category: patch.category.as_deref(),
                    plugin: Some(&name),
                }),
            users,
        );
        for CatalogPatch {
            display, drum_kit, ..
        } in patches
        {
            entries.push(Entry {
                role: index.role_of(&display),
                kind: index.preset_label_of(&display),
                id: id.clone(),
                plugin: name.clone(),
                patch: display,
                drum_kit,
            });
        }
    }
    entries
}

fn filter(entries: &[Entry], role: PatchRole, kind: &str) -> Value {
    entries
        .iter()
        .filter(|e| e.role == Some(role) && e.kind == Some(kind))
        .map(|e| json!({"format":"CLAP", "pluginId":e.id, "plugin":e.plugin, "patch":e.patch}))
        .collect()
}

#[cfg(test)]
mod tests;
