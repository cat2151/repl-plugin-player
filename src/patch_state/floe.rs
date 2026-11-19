//! Floe patches, loaded into the running instance with Floe's own CLAP extension
//! instead of a state file.
use super::clap::Plugin;
use crate::patch_reference::{PatchReference, Source};
use std::path::Path;

/// The Floe preset `reference` restores, when it is one.
pub(super) fn preset(reference: &PatchReference) -> Option<&Path> {
    match &reference.source {
        Source::Catalog { path, .. }
            if reference.plugin.format == "CLAP"
                && reference.plugin.id == cmrt_server_config::FLOE_PLUGIN_ID =>
        {
            Some(path)
        }
        _ => None,
    }
}

/// Fails as [`load`] would on a preset that cannot be read, without an instance.
pub(super) fn check(path: &Path) -> Result<(), String> {
    std::fs::metadata(path)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Asks the plugin to load the preset. Floe finishes on its main thread
/// callbacks, which the control thread keeps running after this returns.
pub(super) fn load(plugin: Plugin, path: &Path) -> Result<(), String> {
    // SAFETY: `Plugin` is a live instance, used on the control (CLAP main) thread.
    unsafe { cmrt_core::floe::request_preset_load(plugin.raw().cast(), path) }
        .map_err(|e| format!("{e:#}"))
}
