//! Surge XT Effects states prepared ahead, from the table cat-plugin-player
//! keeps in its cache: the state for a `.srgfx` preset, keyed by the SHA-256
//! of the file.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::Path, sync::OnceLock};

pub(super) const PLUGIN_ID: &str = "org.surge-synth-team.surge-xt-fx";

#[derive(Deserialize)]
struct Table {
    plugin_id: String,
    states: HashMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    /// Hex of the state.
    state: String,
}

/// Read once for the process; `None` when the player has not saved one.
static TABLE: OnceLock<Option<Table>> = OnceLock::new();

fn read() -> Option<Table> {
    let path = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)?
        .join("cat-plugin-player/patch-catalog/surge-xt-fx-states.json");
    parse(&std::fs::read(path).ok()?)
}

fn parse(bytes: &[u8]) -> Option<Table> {
    let table: Table = serde_json::from_slice(bytes).ok()?;
    (table.plugin_id == PLUGIN_ID).then_some(table)
}

/// The prepared state for the preset file `path`; `None` when the table lacks
/// it and the player must make it.
pub(super) fn state(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    lookup(TABLE.get_or_init(read).as_ref(), &bytes)
}

fn lookup(table: Option<&Table>, preset: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let key: String = Sha256::digest(preset)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    match table.and_then(|t| t.states.get(&key)) {
        Some(entry) => crate::control::decode_hex(&entry.state).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preset_in_the_table_is_its_state_and_others_are_missing() {
        let key: String = Sha256::digest(b"preset")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let json = format!(
            r#"{{"format_version":1,"plugin_id":"{PLUGIN_ID}","states":{{"{key}":{{"path":"p.srgfx","state":"0aff"}}}}}}"#
        );
        let table = parse(json.as_bytes()).unwrap();
        assert_eq!(lookup(Some(&table), b"preset"), Ok(Some(vec![0x0a, 0xff])));
        assert_eq!(lookup(Some(&table), b"other"), Ok(None));
        assert_eq!(lookup(None, b"preset"), Ok(None));
        assert!(parse(json.replace(PLUGIN_ID, "other").as_bytes()).is_none());
    }
}
