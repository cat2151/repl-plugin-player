//! Effect preset states built on the plugin's template, as cmrt loads the same
//! presets into its own instances.
use cmrt_core::{
    dragonfly_preset::{dragonfly_plugin, dragonfly_state_blob, DragonflyPlugin, DragonflyPreset},
    shu_preset::{shu_preset, shu_state_blob, ShuPreset, SHU_PLUGIN_ID},
    tone3000_preset::{parse_t3k_preset, tone3000_state_blob, Tone3000Preset, TONE3000_PLUGIN_ID},
    voyage_voyage_preset::{parse_voyage_xml, voyage_state_blob, VOYAGE_VOYAGE_PLUGIN_ID},
};
use std::path::Path;

pub(super) fn builds(plugin_id: &str) -> bool {
    dragonfly_plugin(plugin_id).is_some()
        || [SHU_PLUGIN_ID, TONE3000_PLUGIN_ID, VOYAGE_VOYAGE_PLUGIN_ID].contains(&plugin_id)
}

/// The preset `value` (built-in presets) or the preset file `path` (the rest).
enum Preset {
    Dragonfly(&'static DragonflyPlugin, &'static DragonflyPreset),
    Shu(&'static ShuPreset),
    Tone3000(Tone3000Preset),
    Voyage(String),
}

fn preset(plugin_id: &str, path: &Path, value: &str) -> Result<Preset, String> {
    let text = |e: &dyn std::fmt::Display| format!("{e:#}");
    if let Some(plugin) = dragonfly_plugin(plugin_id) {
        let preset = plugin.preset(value).map_err(|e| text(&e))?;
        return Ok(Preset::Dragonfly(plugin, preset));
    }
    let read = || std::fs::read(path).map_err(|e| e.to_string());
    match plugin_id {
        SHU_PLUGIN_ID => shu_preset(value).map(Preset::Shu).map_err(|e| text(&e)),
        TONE3000_PLUGIN_ID => parse_t3k_preset(&read()?)
            .map(Preset::Tone3000)
            .map_err(|e| text(&e)),
        VOYAGE_VOYAGE_PLUGIN_ID => {
            let xml = String::from_utf8(read()?).map_err(|e| e.to_string())?;
            parse_voyage_xml(&xml).map_err(|e| text(&e))?;
            Ok(Preset::Voyage(xml))
        }
        other => Err(format!("no preset format for '{other}'")),
    }
}

pub(super) fn check(plugin_id: &str, path: &Path, value: &str) -> Result<(), String> {
    preset(plugin_id, path, value)
        .map(|_| ())
        .map_err(|e| failed(path, value, e))
}

/// A TONE3000 state reads its model after the load, so the effect is silent
/// for a while after this state is loaded.
pub(super) fn build(
    template: &[u8],
    plugin_id: &str,
    path: &Path,
    value: &str,
) -> Result<Vec<u8>, String> {
    let built = preset(plugin_id, path, value).and_then(|preset| {
        match preset {
            Preset::Dragonfly(plugin, preset) => dragonfly_state_blob(template, plugin, preset),
            Preset::Shu(preset) => shu_state_blob(template, preset),
            Preset::Tone3000(preset) => tone3000_state_blob(template, &preset),
            Preset::Voyage(xml) => voyage_state_blob(template, &xml),
        }
        .map_err(|e| format!("{e:#}"))
    });
    built.map_err(|e| failed(path, value, e))
}

fn failed(path: &Path, value: &str, error: String) -> String {
    format!("{} ({value}): {error}", path.display())
}
