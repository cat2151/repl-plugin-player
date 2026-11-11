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
    editable: bool,
) -> Result<(), String> {
    if !editable && messages.keys().ne(reference.keys()) {
        return Err(format!(
            "{}: translation keys do not match this executable",
            path.display()
        ));
    }
    for (key, text) in messages {
        let expected = reference
            .get(key)
            .ok_or_else(|| format!("{}: unknown translation key: {key}", path.display()))?;
        let actual = placeholders(text).map_err(|e| format!("{}: {key}: {e}", path.display()))?;
        if actual != placeholders(expected)? {
            return Err(format!("{}: placeholder mismatch: {key}", path.display()));
        }
    }
    Ok(())
}

impl Catalog {
    pub fn load(directory: &Path, language: &str) -> Result<Self, String> {
        Self::load_at(directory, language, false)
    }

    pub fn load_editable(directory: &Path, language: &str) -> Result<Self, String> {
        Self::load_at(directory, language, true)
    }

    fn load_at(directory: &Path, language: &str, editable: bool) -> Result<Self, String> {
        let en = parse(EMBEDDED[0].1, Path::new("embedded en.toml"))?;
        let ja = parse(EMBEDDED[1].1, Path::new("embedded ja.toml"))?;
        validate(&ja, &en, Path::new("embedded ja.toml"), false)?;
        let mut selected = BTreeMap::new();
        // Validate both languages before rendering, including the unselected catalog.
        for (lang, mut defaults) in [("en", en), ("ja", ja)] {
            let path = directory.join(format!("{lang}.toml"));
            let source =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let messages = parse(&source, &path)?;
            validate(&messages, &defaults, &path, editable)?;
            defaults.extend(messages);
            if lang == language {
                selected = defaults;
            }
        }
        Ok(Self { messages: selected })
    }
}
