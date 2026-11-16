//! Read the same cache and exclusive Role / Kind classifier as cat-plugin-player.
use cmrt_patches::{PatchRole, PatchRoleIndex, PatchRoleInput};
use cmrt_tui_core::patch_catalog_cache;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};

type Groups = BTreeMap<(String, String), Vec<(String, Option<String>)>>;

pub fn list(role: &str, kind: &str) -> Result<Value, String> {
    let role = PatchRole::ALL
        .into_iter()
        .find(|r| r.key() == role)
        .ok_or_else(|| format!("Unknown catalog role: {role}"))?;
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or("LOCALAPPDATA is unavailable")?;
    let player = local.join("cat-plugin-player");
    let loaded = if patch_catalog_cache::cache_file_path().is_some_and(|p| p.exists()) {
        patch_catalog_cache::load()
    } else {
        patch_catalog_cache::load_from(&player.join("patch-catalog/surge-xt.json"))
    }
    .map_err(|e| format!("{e:#}"))?;
    let users: Vec<(String, String)> = match std::fs::read(player.join("patch-filter-presets.json"))
    {
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
            || !std::path::Path::new(&plugin.plugin_path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("clap"))
        {
            continue;
        }
        groups
            .entry((id.to_owned(), plugin.name.clone()))
            .or_default()
            .push((
                patch.reference.display.clone(),
                patch.selector_category.clone(),
            ));
    }
    Ok(select(groups, &users, role, kind))
}

fn select(groups: Groups, users: &[(String, String)], role: PatchRole, kind: &str) -> Value {
    let mut result = Vec::new();
    for ((id, name), patches) in groups {
        let normalized: Vec<_> = patches
            .iter()
            .map(|(display, _)| display.to_lowercase())
            .collect();
        let index = PatchRoleIndex::build(
            patches
                .iter()
                .zip(&normalized)
                .map(|((display, category), n)| PatchRoleInput {
                    display,
                    normalized_display: n,
                    selector_category: category.as_deref(),
                    plugin: Some(&name),
                }),
            users,
        );
        for (display, _) in patches {
            if index.role_of(&display) == Some(role)
                && index.preset_label_of(&display) == Some(kind)
            {
                result
                    .push(json!({"format":"CLAP", "pluginId":id, "plugin":name, "patch":display}));
            }
        }
    }
    json!(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_kind_uses_exclusive_roles_and_keeps_plugin_identity() {
        let groups = [
            (
                ("alpha".into(), "Alpha".into()),
                vec![
                    ("Pads/Soft".into(), None),
                    ("Bass/Pad Bass".into(), None),
                    ("Leads/Pad Lead".into(), None),
                ],
            ),
            (
                ("beta".into(), "Beta".into()),
                vec![("Pads/Soft".into(), None)],
            ),
        ]
        .into();
        let selected = select(groups, &[], PatchRole::Chord, "pad");
        assert_eq!(
            selected,
            json!([
                {"format":"CLAP","pluginId":"alpha","plugin":"Alpha","patch":"Pads/Soft"},
                {"format":"CLAP","pluginId":"alpha","plugin":"Alpha","patch":"Leads/Pad Lead"},
                {"format":"CLAP","pluginId":"beta","plugin":"Beta","patch":"Pads/Soft"}
            ])
        );
    }
}
