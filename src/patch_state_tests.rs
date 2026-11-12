use super::*;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("repl-patch-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn file(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"x").unwrap();
        path
    }
    fn root(&self, relative: &str) -> String {
        let path = self.0.join(relative);
        std::fs::create_dir_all(&path).unwrap();
        path.to_str().unwrap().into()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_program_inside_a_file_names_the_file_and_keeps_the_selector() {
    let dir = Dir::new("program");
    let cartridge = dir.file("Cartridges/Set/Bank.syx");
    let root = dir.root("Cartridges");
    let (path, file) = locate(&[root], "Set/Bank.syx/09 Voice").unwrap();
    assert_eq!(file, cartridge);
    assert_eq!(path, cartridge.join("09 Voice"));
}

#[test]
fn a_patch_file_is_both_path_and_file() {
    let dir = Dir::new("file");
    let patch = dir.file("factory/Basses/Attacky.fxp");
    let (path, file) = locate(&[dir.root("factory")], "Basses/Attacky.fxp").unwrap();
    assert_eq!((path, file), (patch.clone(), patch));
}

#[test]
fn a_display_path_may_start_with_the_roots_own_name() {
    let dir = Dir::new("own");
    let patch = dir.file("Free Sounds/Programs/main.sfz");
    let roots = [dir.root("Free Sounds"), dir.root("Elsewhere")];
    let (path, _) = locate(&roots, "Free Sounds/Programs/main.sfz").unwrap();
    assert_eq!(path, patch);
}

#[test]
fn missing_folders_and_several_matches_are_refused() {
    let dir = Dir::new("refused");
    dir.file("a/same.fxp");
    dir.file("b/same.fxp");
    dir.file("a/Folder/inner.fxp");
    let roots = [dir.root("a"), dir.root("b")];
    assert_eq!(locate(&roots, "same.fxp"), Err("patch_state_01"));
    assert_eq!(locate(&roots, "missing.fxp"), Err("patch_state_00"));
    assert_eq!(locate(&roots, "Folder"), Err("patch_state_00"));
    assert_eq!(locate(&roots, "Folder/missing/x"), Err("patch_state_00"));
    assert_eq!(locate(&roots, "../a/same.fxp"), Err("patch_state_00"));
    assert_eq!(locate(&roots, "C:/a/same.fxp"), Err("patch_state_00"));
}

#[test]
fn the_reference_takes_the_bundle_from_the_players_table() {
    let dir = Dir::new("reference");
    let patch = dir.file("root/p.fxp");
    let table = Table {
        plugins: vec![Plugin {
            format: "CLAP".into(),
            id: "id".into(),
            bundle_path: "D:/plugin.clap".into(),
            roots: vec![dir.root("root")],
        }],
    };
    let found = reference(&table, "CLAP", "id", "p.fxp").unwrap();
    assert_eq!(found.plugin.bundle_path, "D:/plugin.clap");
    assert_eq!(
        found.source,
        Source::Catalog {
            path: patch.clone(),
            file: patch
        }
    );
    assert_eq!(found.display, "p.fxp");
    assert!(found.validate().is_ok());
    assert!(reference(&table, "VST3", "id", "p.fxp").is_err());
}
