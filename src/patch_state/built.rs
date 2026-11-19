//! Patch and effect preset states built on the state a new instance of the
//! plugin saved (its template), kept once per plugin for the process.
use crate::patch_reference::Source;
use cmrt_core::sforzando::{sforzando_ariax_state_blob, sforzando_state_blob, SfzStreaming};
use cmrt_server_config::{resolve_sforzando_preset, resolve_sforzando_program};
use std::{path::Path, sync::Mutex};

pub(super) use cmrt_server_config::SFORZANDO_PLUGIN_ID;

mod effect;

/// The plugin ID and template of each plugin given one.
static TEMPLATES: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::new());

/// Whether the CLAP plugin's state for `source` is built on a template.
pub(super) fn builds(plugin_id: &str, source: &Source) -> bool {
    match source {
        Source::Catalog { .. } => plugin_id == SFORZANDO_PLUGIN_ID,
        Source::Effect { .. } => effect::builds(plugin_id),
        Source::Imported { .. } => false,
    }
}

/// Keeps `template` unless the plugin already has one.
pub(super) fn keep(plugin_id: &str, template: Vec<u8>) {
    let mut kept = TEMPLATES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !kept.iter().any(|(id, _)| id == plugin_id) {
        kept.push((plugin_id.into(), template));
    }
}

pub(super) fn kept(plugin_id: &str) -> Option<Vec<u8>> {
    let kept = TEMPLATES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    kept.iter()
        .find(|(id, _)| id == plugin_id)
        .map(|(_, template)| template.clone())
}

/// Fails as [`build`] would on a patch or preset it cannot find, without a
/// template.
pub(super) fn check(plugin_id: &str, source: &Source) -> Result<(), String> {
    match source {
        Source::Effect { path, value } => effect::check(plugin_id, path, value),
        Source::Catalog { path, .. } => patch(path).map(|_| ()),
        Source::Imported { .. } => Err("an imported state is not built".into()),
    }
}

/// The state for `source` built on the plugin's `template`.
pub(super) fn build(template: &[u8], plugin_id: &str, source: &Source) -> Result<Vec<u8>, String> {
    let path = match source {
        Source::Effect { path, value } => return effect::build(template, plugin_id, path, value),
        Source::Catalog { path, .. } => path,
        Source::Imported { .. } => return Err("an imported state is not built".into()),
    };
    let streaming = SfzStreaming::PluginDefault;
    match patch(path)? {
        Patch::Program(program) => sforzando_state_blob(template, &program, streaming),
        Patch::Preset(xml) => sforzando_ariax_state_blob(template, &xml, streaming),
    }
    .map_err(|e| format!("{}: {e:#}", path.display()))
}

enum Patch {
    Program(cmrt_server_config::SforzandoProgramRef),
    Preset(String),
}

fn patch(path: &Path) -> Result<Patch, String> {
    let ariax = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ariax"));
    if ariax {
        resolve_sforzando_preset(path).map(|preset| Patch::Preset(preset.xml))
    } else {
        resolve_sforzando_program(path).map(Patch::Program)
    }
    .map_err(|e| format!("{}: {e:#}", path.display()))
}
