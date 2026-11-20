//! The cell a patch or effect preset chosen in the player's browser becomes:
//! code the user could have typed, naming the plugin and the preset, so that
//! running the cell alone restores the sound.
use crate::{patch_reference::Source, resident_ipc::Selected, snippets::Snippet};

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
        keywords: String::new(),
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

/// Makes the effect preset one of the effects of `track`, the track that was
/// browsed. The preset is its file, relative to the plugin's root, and the
/// player's selector inside that file.
pub fn effect(selected: &Selected, track: i64) -> Result<Snippet, String> {
    let plugin = &selected.plugin;
    let Some(Source::Effect { path, value }) = selected.reference.as_ref().map(|r| &r.source)
    else {
        return Err(crate::i18n::t("patch_browse_04").into());
    };
    let preset = crate::patch_state::effect_preset(&plugin.format, &plugin.id, path)?;
    let name = match &selected.preset_name {
        Some(name) => format!("{} / {name}", plugin.name),
        None => plugin.name.clone(),
    };
    Ok(Snippet {
        name: name.replace(['\r', '\n'], " "),
        group: "Effect",
        keywords: String::new(),
        code: format!(
            "{{
    const track = {track};
    include('builtin/lib/effect.js').use({{format:{}, plugin:{}, preset:{}, value:{}, track}});
}}",
            literal(&plugin.format),
            literal(&plugin.name),
            literal(&preset),
            literal(value)
        ),
    })
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
