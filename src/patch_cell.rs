//! The cell a patch chosen in the patch browser becomes: code the user could
//! have typed, naming the plugin and the patch's path in the player's catalog,
//! so that running the cell alone restores the sound.
use crate::{resident_ipc::Selected, snippets::Snippet};

/// Sets the instrument of `track`, the track that was browsed.
pub fn snippet(selected: &Selected, track: i64) -> Snippet {
    let plugin = &selected.plugin;
    let patch = selected.preset_name.clone();
    let name = match &patch {
        Some(patch) => format!("{} / {patch}", plugin.name),
        None => plugin.name.clone(),
    };
    let patch = patch.map_or(String::new(), |p| format!(", patch:{}", literal(&p)));
    Snippet {
        name: name.replace(['\r', '\n'], " "),
        group: "Patch",
        code: format!(
            "{{
    const track = {track};
    include('builtin/lib/patch.js').use({{format:{}, plugin:{}{patch}, track}});
}}",
            literal(&plugin.format),
            literal(&plugin.name)
        ),
    }
}

/// A single-quoted JavaScript string literal.
fn literal(text: &str) -> String {
    let mut out = String::from("'");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
#[path = "patch_cell_tests.rs"]
mod tests;
