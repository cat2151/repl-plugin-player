use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "repl-snippets-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn library(&self) -> Library {
        Library {
            builtin: self.0.join("builtin"),
            user: Some(self.0.join("user")),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let target = self.0.canonicalize().unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap();
        assert!(target.starts_with(&root) && target != root);
        fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn builtins_reload_additions_and_edits_without_a_build() {
    let fixture = Fixture::new();
    let library = fixture.library();
    fs::create_dir(&library.builtin).unwrap();
    fs::write(library.builtin.join("try-play.js"), "var start = 1;").unwrap();
    assert_eq!(library.initial().unwrap(), "var start = 1;");
    assert_eq!(library.load().unwrap().len(), 1);
    fs::write(library.builtin.join("猫.js"), "var cat = '😀';").unwrap();
    fs::write(library.builtin.join("try-play.js"), "var start = 2;").unwrap();
    fs::write(library.builtin.join("ignored.txt"), "ignored").unwrap();
    assert_eq!(library.load().unwrap().len(), 2);
    assert_eq!(library.initial().unwrap(), "var start = 2;");
    assert!(library
        .load()
        .unwrap()
        .iter()
        .any(|s| s.name == "猫" && s.code.contains('😀')));
}

#[test]
fn embedded_catalog_includes_startup_script_when_repository_is_absent() {
    let fixture = Fixture::new();
    let library = fixture.library();
    assert_eq!(
        library.initial().unwrap(),
        library.source(STARTUP_SOURCE).unwrap()
    );
    assert!(library
        .load()
        .unwrap()
        .iter()
        .any(|s| s.name == "try-play" && s.code == library.source(STARTUP_SOURCE).unwrap()));
    assert_eq!(library.load().unwrap().len(), embedded_entries());
}

/// Number of embedded built-ins that belong in the list (libraries excluded).
fn embedded_entries() -> usize {
    EMBEDDED
        .iter()
        .filter(|(path, _)| !path.contains('/'))
        .count()
}

#[test]
fn includes_resolve_namespaces_reread_edits_and_keep_libraries_off_the_list() {
    let fixture = Fixture::new();
    let library = fixture.library();
    for root in [&library.builtin, library.user.as_ref().unwrap()] {
        fs::create_dir_all(root.join("lib")).unwrap();
        fs::write(root.join("example.js"), "0").unwrap();
    }
    let path = library.builtin.join("lib/猫.js");
    fs::write(&path, "'builtin'").unwrap();
    fs::write(library.user.as_ref().unwrap().join("lib/猫.js"), "'user'").unwrap();
    assert_eq!(library.source("builtin/lib/猫.js").unwrap(), "'builtin'");
    assert_eq!(library.source("user/lib/猫.js").unwrap(), "'user'");
    fs::write(&path, "'edited'").unwrap();
    assert_eq!(library.source("builtin/lib/猫.js").unwrap(), "'edited'");
    assert!(library.load().unwrap().iter().all(|s| s.name == "example"));
    fs::write(&path, [0xff]).unwrap();
    assert!(library
        .source("builtin/lib/猫.js")
        .unwrap_err()
        .contains("builtin/lib/猫.js"));
}

#[test]
fn embedded_include_libraries_are_available_but_external_catalogs_are_authoritative() {
    let fixture = Fixture::new();
    let library = fixture.library();
    let libraries: Vec<_> = EMBEDDED
        .iter()
        .filter(|(path, _)| path.starts_with("lib/"))
        .collect();
    assert!(!libraries.is_empty());
    for (path, embedded) in &libraries {
        let code = library.source(&format!("builtin/{path}")).unwrap();
        assert_eq!(code, localize_builtin(embedded));
        assert!(!code.contains("@text "));
    }
    assert_eq!(library.load().unwrap().len(), embedded_entries());
    assert!(!library.load().unwrap().iter().any(|s| s.name.contains('/')));
    fs::create_dir(&library.builtin).unwrap();
    let (path, _) = libraries[0];
    assert!(library.source(&format!("builtin/{path}")).is_err());
    assert!(library.source(&format!("user/{path}")).is_err());
}

#[test]
fn includes_reject_ambiguous_paths_and_report_the_requested_name() {
    let fixture = Fixture::new();
    let library = fixture.library();
    for name in [
        "example.js",
        "other/example.js",
        "builtin/../example.js",
        "builtin/./example.js",
        "builtin//example.js",
        "builtin/lib/",
        "builtin/lib/a.txt",
        "builtin/lib/a.js:stream.js",
        "builtin/X:/a.js",
        "X:/a.js",
        "builtin/lib\\a.js",
        "builtin/missing.js",
        "builtin/lib/CON.js",
    ] {
        assert!(library.source(name).unwrap_err().contains(name), "{name}");
    }
}

#[test]
fn saved_snippets_survive_reopening_and_never_replace_existing_files() {
    let fixture = Fixture::new();
    let library = fixture.library();
    let code = "var sound = '猫😀';\n";
    let path = library.save("my sound", code).unwrap();
    let reopened = fixture.library();
    let entries = reopened.load().unwrap();
    assert!(entries
        .iter()
        .any(|s| s.name == "my sound" && s.group == "User" && s.code == code));
    assert!(reopened.save("my sound", "replacement").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), code);
    for name in [
        "../outside",
        "a/b",
        "a\\b",
        "CON",
        "nul.txt",
        "",
        "..",
        "name.",
        "a\nb",
    ] {
        assert!(library.save(name, code).is_err(), "{name}");
    }
    assert!(library.save("empty", " \n").is_err());
}

#[test]
fn unreadable_files_and_missing_initial_are_errors_instead_of_embedded_fallback() {
    let fixture = Fixture::new();
    let library = fixture.library();
    fs::create_dir(&library.builtin).unwrap();
    assert!(library.initial().is_err());
    fs::write(library.builtin.join("broken.js"), [0xff, 0xfe]).unwrap();
    assert!(library.load().unwrap_err().contains("broken.js"));
}

#[test]
fn builtin_location_uses_this_executables_project_or_packaged_directory() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    fs::create_dir_all(repo.join("locales")).unwrap();
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = 'repl-plugin-player'",
    )
    .unwrap();
    assert_eq!(
        builtin_directory(&repo.join("target/release")),
        repo.join("snippets/builtin")
    );
    let package = fixture.0.join("package");
    fs::create_dir(&package).unwrap();
    assert_eq!(
        builtin_directory(&package),
        package.join("snippets/builtin")
    );
    let library = Library {
        builtin: builtin_directory(&package),
        user: None,
    };
    let entries = library.load().unwrap();
    assert!(entries.iter().any(|entry| entry.name == "try-play"));
    assert_eq!(entries.len(), embedded_entries());
    assert!(entries.iter().all(|entry| !entry.code.contains("@text ")));
}
