use super::*;

#[test]
fn language_override_is_common_and_validated() {
    let args = ["app", "--automation", "--lang", "ja", "--timeout", "3"].map(String::from);
    let (remaining, lang) = arguments(&args, "en").unwrap();
    assert_eq!(lang, "ja");
    assert_eq!(remaining, ["app", "--automation", "--timeout", "3"]);
    for args in [
        vec!["--lang"],
        vec!["--lang", "fr"],
        vec!["--lang", "ja", "--lang", "en"],
    ] {
        assert!(arguments(
            &args.into_iter().map(String::from).collect::<Vec<_>>(),
            "en"
        )
        .is_err());
    }
    assert!(is_japanese(0x0411));
    assert!(!is_japanese(0x0409));
}

#[test]
fn catalogs_and_resource_layouts_fail_closed() {
    let root = std::env::temp_dir().join(format!("repl-locales-{}", std::process::id()));
    std::fs::create_dir_all(root.join("target/release")).unwrap();
    std::fs::create_dir_all(root.join("locales")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'repl-plugin-player'",
    )
    .unwrap();
    for (lang, source) in catalog::EMBEDDED {
        std::fs::write(root.join(format!("locales/{lang}.toml")), source).unwrap();
    }
    for start in [
        root.clone(),
        root.join("target/release"),
        root.join("target/debug"),
    ] {
        assert_eq!(crate::resources::project_root(&start).unwrap(), root);
    }
    assert!(Catalog::load(&root.join("locales"), "ja").is_ok());
    std::fs::write(root.join("locales/ja.toml"), "m000 = 'custom'").unwrap();
    assert!(Catalog::load(&root.join("locales"), "ja").is_err());
    let embedded = |lang: &str| {
        let source = catalog::EMBEDDED
            .iter()
            .find(|(l, _)| *l == lang)
            .unwrap()
            .1;
        toml::from_str::<BTreeMap<String, String>>(source).unwrap()
    };
    let editable = Catalog::load_editable(&root.join("locales"), "ja").unwrap();
    assert_eq!(editable.messages["m000"], "custom");
    let mut expected = embedded("ja");
    expected.insert("m000".into(), "custom".into());
    assert_eq!(editable.messages, expected);
    // Keys this exe no longer has, or whose placeholders changed, fall back to the embedded text.
    std::fs::write(
        root.join("locales/ja.toml"),
        "m000 = 'custom'\nm016 = 'old'\nf000 = '{$other}'\nm001 = '{$open'",
    )
    .unwrap();
    let editable = Catalog::load_editable(&root.join("locales"), "ja").unwrap();
    assert_eq!(editable.messages, expected);
    assert!(!editable.messages.contains_key("m016"));
    for lang in ["ja", "en"] {
        std::fs::write(root.join("locales/ja.toml"), "").unwrap();
        std::fs::write(root.join("locales/en.toml"), "").unwrap();
        let empty = Catalog::load_editable(&root.join("locales"), lang).unwrap();
        assert_eq!(empty.messages, embedded(lang));
    }
    for (lang, source) in catalog::EMBEDDED {
        std::fs::write(root.join(format!("locales/{lang}.toml")), source).unwrap();
    }
    let mut changed = embedded("ja");
    changed.insert("f000".into(), "{$other}".into());
    std::fs::write(
        root.join("locales/ja.toml"),
        toml::to_string(&changed).unwrap(),
    )
    .unwrap();
    assert!(Catalog::load(&root.join("locales"), "ja")
        .unwrap_err_string()
        .contains("placeholder"));
    let reference = root.join("locales/ja.default.toml");
    crate::resources::write_replacing(&reference, b"full").unwrap();
    std::fs::write(&reference, "edited").unwrap();
    crate::resources::write_replacing(&reference, b"full").unwrap();
    assert_eq!(std::fs::read(&reference).unwrap(), b"full");
    std::fs::write(root.join("locales/ja.toml"), "broken = [").unwrap();
    assert!(Catalog::load_editable(&root.join("locales"), "en").is_err());
    // A simultaneous publisher and later startup must preserve the user's file.
    let path = root.join("locales/ja.toml");
    let before = std::fs::read(&path).unwrap();
    crate::resources::write_missing(&path, b"replacement").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(crate::resources::project_root(&root).is_some());
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = 'other-project'").unwrap();
    assert!(crate::resources::project_root(&root.join("target/release")).is_none());
    let resolved = root.canonicalize().unwrap();
    let temporary = std::env::temp_dir().canonicalize().unwrap();
    assert!(resolved.starts_with(&temporary) && resolved != temporary);
    std::fs::remove_dir_all(resolved).unwrap();
}

trait ErrorText {
    fn unwrap_err_string(self) -> String;
}
impl ErrorText for Result<Catalog, String> {
    fn unwrap_err_string(self) -> String {
        match self {
            Err(error) => error,
            Ok(_) => panic!("expected error"),
        }
    }
}

#[test]
fn checked_in_catalogs_have_identical_contracts() {
    for language in ["ja", "en"] {
        let catalog = Catalog::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("locales"),
            language,
        )
        .unwrap();
        assert!(catalog.messages.len() >= 90);
    }
    assert_eq!(
        render("f000", &[("error", "literal {$error}".into())]),
        "Initialization: literal {$error}"
    );
}

#[test]
fn script_translation_arguments_are_named_and_validated() {
    use serde_json::json;
    assert_eq!(
        formatted("snippet_14", &json!({"clipId":12})).unwrap(),
        "Could not remove MIDI clip 12"
    );
    assert!(formatted("snippet_14", &json!({})).is_err());
    assert!(formatted("snippet_14", &json!({"clipId":{},"extra":1})).is_err());
    // Arguments are inserted once: a value that looks like a placeholder stays literal.
    let inserted = formatted("snippet_14", &json!({"clipId":"literal {$clipId}"})).unwrap();
    assert!(inserted.contains("literal {$clipId}"));
    assert_eq!(inserted.matches("{$").count(), 1);
}
