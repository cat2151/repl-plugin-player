//! Where a display path names a patch under the patch folders of the player's table.
use super::Table;
use crate::{
    host::PluginKey,
    i18n::render,
    patch_reference::{PatchReference, Source},
};
use std::path::{Path, PathBuf};

pub(super) fn reference(
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
pub(super) fn locate(roots: &[String], display: &str) -> Result<(PathBuf, PathBuf), &'static str> {
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
