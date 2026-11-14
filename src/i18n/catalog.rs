use super::*;

pub(super) const EMBEDDED: [(&str, &str); 2] = [
    ("en", include_str!("../../locales/en.toml")),
    ("ja", include_str!("../../locales/ja.toml")),
];

fn parse(source: &str, path: &Path) -> Result<BTreeMap<String, String>, String> {
    toml::from_str(source).map_err(|e| format!("{}: {e}", path.display()))
}

fn validate(
    messages: &BTreeMap<String, String>,
    reference: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), String> {
    if messages.keys().ne(reference.keys()) {
        return Err(format!(
            "{}: translation keys do not match this executable",
            path.display()
        ));
    }
    for (key, text) in messages {
        let actual = placeholders(text).map_err(|e| format!("{}: {key}: {e}", path.display()))?;
        if actual != placeholders(&reference[key])? {
            return Err(format!("{}: placeholder mismatch: {key}", path.display()));
        }
    }
    Ok(())
}

/// A user override is kept only while this exe still has the key with the same placeholders,
/// so updating the exe never stops startup over a renamed key or a changed `{$…}`.
fn matches_embedded(reference: &BTreeMap<String, String>, key: &str, text: &str) -> bool {
    reference
        .get(key)
        .is_some_and(|expected| placeholders(text).ok() == placeholders(expected).ok())
}

impl Catalog {
    /// The source tree must match this exe exactly.
    pub fn load(directory: &Path, language: &str) -> Result<Self, String> {
        Self::load_at(directory, language, false)
    }

    /// User files hold only the keys the user changed, laid over the embedded catalog.
    pub fn load_editable(directory: &Path, language: &str) -> Result<Self, String> {
        Self::load_at(directory, language, true)
    }

    fn load_at(directory: &Path, language: &str, editable: bool) -> Result<Self, String> {
        let en = parse(EMBEDDED[0].1, Path::new("embedded en.toml"))?;
        let ja = parse(EMBEDDED[1].1, Path::new("embedded ja.toml"))?;
        validate(&ja, &en, Path::new("embedded ja.toml"))?;
        let mut selected = BTreeMap::new();
        let mut english = BTreeMap::new();
        // Validate both languages before rendering, including the unselected catalog.
        for (lang, mut defaults) in [("en", en), ("ja", ja)] {
            let path = directory.join(format!("{lang}.toml"));
            let source =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut messages = parse(&source, &path)?;
            if editable {
                messages.retain(|key, text| matches_embedded(&defaults, key, text));
            } else {
                validate(&messages, &defaults, &path)?;
            }
            defaults.extend(messages);
            if lang == "en" {
                english = defaults.clone();
            }
            if lang == language {
                selected = defaults;
            }
        }
        Ok(Self {
            language: language.to_owned(),
            messages: selected,
            english,
        })
    }
}
