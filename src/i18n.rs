//! Both catalogs are validated before native initialization or the first frame.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::OnceLock,
};

pub struct Catalog {
    language: String,
    messages: BTreeMap<String, String>,
}
mod catalog;
static CATALOG: OnceLock<Catalog> = OnceLock::new();

fn placeholders(text: &str) -> Result<BTreeSet<String>, String> {
    let mut result = BTreeSet::new();
    let mut rest = text;
    while let Some((_, tail)) = rest.split_once("{$") {
        let (name, tail) = tail.split_once('}').ok_or("Unclosed placeholder")?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("Invalid placeholder: {name}"));
        }
        result.insert(name.to_owned());
        rest = tail;
    }
    Ok(result)
}

pub fn is_japanese(language: u16) -> bool {
    language & 0x3ff == 0x11
}

fn os_language() -> &'static str {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    if is_japanese(unsafe { GetUserDefaultUILanguage() }) {
        "ja"
    } else {
        "en"
    }
}

pub fn arguments(args: &[String], default: &str) -> Result<(Vec<String>, String), String> {
    let mut result = Vec::new();
    let mut language = default.to_owned();
    let mut seen = false;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--lang" {
            let value = args
                .get(i + 1)
                .filter(|v| matches!(v.as_str(), "ja" | "en"))
                .ok_or("--lang requires ja or en")?;
            if seen {
                return Err("Specify --lang only once (ja or en)".into());
            }
            seen = true;
            language.clone_from(value);
            i += 2;
        } else {
            result.push(args[i].clone());
            i += 1;
        }
    }
    Ok((result, language))
}

pub fn initialize(args: &[String]) -> Result<Vec<String>, String> {
    let (args, language) = arguments(args, os_language())?;
    let catalog = if let Some(root) = crate::resources::source_root() {
        Catalog::load(&root.join("locales"), &language)?
    } else {
        let directory = crate::resources::user_directory()?.join("locales");
        // `<lang>.default.toml` is only a reference for copying keys; reading uses the embedded text.
        for (lang, source) in catalog::EMBEDDED {
            crate::resources::write_replacing(
                &directory.join(format!("{lang}.default.toml")),
                source.as_bytes(),
            )?;
            crate::resources::write_missing(&directory.join(format!("{lang}.toml")), b"")?;
        }
        Catalog::load_editable(&directory, &language)?
    };
    CATALOG
        .set(catalog)
        .map_err(|_| "Language already initialized")?;
    Ok(args)
}

pub fn text(key: &str) -> Result<&'static str, String> {
    let _ = t("m000");
    CATALOG
        .get()
        .unwrap()
        .messages
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| render("unknown_text", &[("key", key.into())]))
}

fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| {
        #[cfg(test)]
        {
            Catalog::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("locales"), "en").unwrap()
        }
        #[cfg(not(test))]
        {
            panic!("Translation used before startup")
        }
    })
}

/// The language chosen at startup, `"ja"` or `"en"`.
pub fn language() -> &'static str {
    &catalog().language
}

pub fn t(key: &str) -> &'static str {
    catalog()
        .messages
        .get(key)
        .unwrap_or_else(|| panic!("Missing translation key: {key}"))
}

pub fn render(key: &str, values: &[(&str, String)]) -> String {
    // One pass: values may contain placeholder-looking user code or diagnostics.
    let text = t(key);
    let mut output = String::new();
    let mut rest = text;
    while let Some((head, tail)) = rest.split_once("{$") {
        output.push_str(head);
        let (name, tail) = tail.split_once('}').expect("Validated placeholder");
        output.push_str(
            &values
                .iter()
                .find(|(key, _)| *key == name)
                .expect("Missing argument")
                .1,
        );
        rest = tail;
    }
    output.push_str(rest);
    output
}

pub fn formatted(key: &str, values: &serde_json::Value) -> Result<String, String> {
    let template = text(key)?;
    let required = placeholders(template)?;
    let object = values
        .as_object()
        .ok_or_else(|| render("text_argument", &[("name", "values".into())]))?;
    let mut arguments = Vec::new();
    for name in &required {
        let value = object
            .get(name)
            .filter(|v| v.is_string() || v.is_number() || v.is_boolean())
            .ok_or_else(|| render("text_argument", &[("name", name.clone())]))?;
        arguments.push((
            name.as_str(),
            value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string()),
        ));
    }
    if let Some(name) = object.keys().find(|name| !required.contains(*name)) {
        return Err(render("text_argument", &[("name", name.clone())]));
    }
    Ok(render(key, &arguments))
}

pub fn group(value: &str) -> &str {
    match value {
        "Built-in" => t("group_builtin"),
        "User" => t("group_user"),
        _ => value,
    }
}
pub fn scope(value: &str) -> &str {
    match value {
        "snippet" => t("scope_snippet"),
        "whole editor" => t("scope_all"),
        "selection" => t("scope_selection"),
        "cell" => t("scope_cell"),
        "line" => t("scope_line"),
        _ => value,
    }
}

pub fn fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into());
    let bytes = ["meiryo.ttc", "msgothic.ttc"]
        .iter()
        .find_map(|name| std::fs::read(Path::new(&windows).join("Fonts").join(name)).ok());
    if let Some(bytes) = bytes {
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert("japanese".into(), FontData::from_owned(bytes).into());
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("japanese".into());
        }
        ctx.set_fonts(fonts);
    }
}

#[cfg(test)]
mod tests;

pub fn snippet_name(name: &str) -> &str {
    match name {
        "try-play" => t("name_try_play"),
        "try-init-saw" => t("name_try_init_saw"),
        "try-chords" => t("name_try_chords"),
        "try-preset" => t("name_try_preset"),
        "try-delay" => t("name_try_delay"),
        "try-surge-ui" => t("name_try_surge_ui"),
        "try-effect-ui" => t("name_try_effect_ui"),
        "try-mml" => t("name_try_mml"),
        "try-loop-mml" => t("name_try_loop_mml"),
        "try-loop-mml-multi-track" => t("name_try_loop_mml_multi_track"),
        "try-instrument" => t("name_try_instrument"),
        "try-parameter-ui" => t("name_try_parameter_ui"),
        "try-patch-browse" => t("name_try_patch_browse"),
        "close-parameter-ui" => t("name_close_parameter_ui"),
        "add-midi-clip" => t("name_add_midi_clip"),
        "play" => t("name_play"),
        "stop-playback" => t("name_stop_playback"),
        "show-instrument-ui" => t("name_show_instrument_ui"),
        "show-effect-ui" => t("name_show_effect_ui"),
        "set-track-volume" => t("name_set_track_volume"),
        "api-list" => t("name_api_list"),
        _ => name,
    }
}
