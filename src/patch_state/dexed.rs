//! Dexed cartridge programs, sent to the running instance as a single voice
//! SysEx instead of a state file.
use crate::patch_reference::{PatchReference, Source};
use cmrt_core::dx7;
use std::path::Path;

/// The cartridge program `reference` restores, when it is one.
pub(super) fn program(reference: &PatchReference) -> Option<&str> {
    match &reference.source {
        Source::Catalog { path, .. }
            if reference.plugin.format == "CLAP"
                && reference.plugin.id == cmrt_server_config::DEXED_PLUGIN_ID =>
        {
            path.to_str().filter(|p| dx7::is_cartridge_patch_path(p))
        }
        _ => None,
    }
}

/// The program's single voice SysEx, read from its cartridge file.
pub(super) fn voice(path: &str) -> Result<Vec<u8>, String> {
    let program = dx7::parse_cartridge_patch_path(path).map_err(|e| format!("{e:#}"))?;
    let file = Path::new(&program.cartridge_path);
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let cartridge =
        dx7::parse_dx7_cartridge(bytes).map_err(|e| format!("{}: {e:#}", file.display()))?;
    Ok(dx7::single_voice_sysex(&cartridge, program.program_index))
}
