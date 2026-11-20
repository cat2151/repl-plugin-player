use super::*;

pub(super) struct Dir(pub(super) PathBuf);

impl Dir {
    pub(super) fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("repl-patch-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    pub(super) fn file(&self, relative: &str) -> PathBuf {
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

#[test]
fn the_owner_tag_of_a_catalog_patch_is_not_part_of_its_path() {
    let dir = Dir::new("tag");
    let patch = dir.file("root/6 Pads/p.h2p");
    let table = Table {
        plugins: vec![Plugin {
            format: "CLAP".into(),
            id: "com.u-he.Zebralette3".into(),
            bundle_path: "D:/plugin.clap".into(),
            roots: vec![dir.root("root")],
        }],
    };
    let found = reference(&table, "CLAP", "com.u-he.Zebralette3", "6 Pads/p.h2p:z").unwrap();
    assert_eq!(
        found.source,
        Source::Catalog {
            path: patch.clone(),
            file: patch
        }
    );
}

pub(super) fn catalog(id: &str, path: PathBuf) -> PatchReference {
    PatchReference {
        plugin: PluginKey {
            format: "CLAP".into(),
            id: id.into(),
            name: String::new(),
            vendor: String::new(),
            bundle_path: "D:/plugin.clap".into(),
        },
        source: Source::Catalog {
            file: path.clone(),
            path,
        },
        display: "p".into(),
    }
}

#[test]
fn converted_formats_are_made_here_without_the_player() {
    let dir = Dir::new("converted");
    let six_sines = dir.0.join("p.sxsnp");
    let patch = br#"<patch id="org.baconpaul.six-sines"><params/></patch>"#;
    std::fs::write(&six_sines, patch).unwrap();
    let made = state(&catalog("org.baconpaul.six-sines", six_sines), None, |_| {
        panic!("asked the player")
    });
    assert_eq!(made.unwrap(), patch);
    for (id, file) in [
        ("org.surge-synth-team.surge-xt", "p.fxp"),
        ("com.u-he.TyrellN6", "p.h2p"),
        ("com.u-he.Zebralette3", "p.h2p"),
        ("com.vastdynamics.VAST2", "p.vvp"),
    ] {
        let invalid = state(&catalog(id, dir.file(file)), None, |_| {
            panic!("asked the player")
        });
        assert!(invalid.is_err(), "{id}");
    }
}

#[test]
fn formats_needing_an_instance_are_asked_of_the_player() {
    let dir = Dir::new("instance");
    for id in NEEDS_INSTANCE {
        let reference = catalog(id, dir.file("p.bin"));
        let mut asked = None;
        let made = state(&reference, None, |input| {
            asked = Some(serde_json::from_slice::<PatchReference>(input).unwrap());
            Ok(vec![1])
        });
        assert_eq!(made.unwrap(), [1]);
        assert_eq!(asked.unwrap(), reference);
        assert!(cmrt_patches::supports_catalog_clap_plugin(id));
    }
}

#[test]
fn a_plugin_with_a_template_builds_on_it_without_the_player() {
    let dir = Dir::new("template");
    let reference = catalog(built::SFORZANDO_PLUGIN_ID, dir.file("p.sfz"));
    let made = state(&reference, Some(b"template"), |_| {
        panic!("asked the player")
    });
    assert!(made.is_err());
}

pub(super) fn effect(id: &str, path: PathBuf, value: &str) -> PatchReference {
    PatchReference {
        source: Source::Effect {
            path,
            value: value.into(),
        },
        ..catalog(id, PathBuf::new())
    }
}

#[test]
fn effect_presets_with_a_template_are_built_without_the_player() {
    let dir = Dir::new("effect");
    let shu = effect("audio.mikey.Shu", dir.file("Shu.clap"), "Ether");
    assert!(templated(&shu));
    let template = vec![0; 35 * 8];
    let made = state(&shu, Some(&template), |_| panic!("asked the player")).unwrap();
    assert_eq!(made.len(), template.len());
    assert_ne!(made, template);
    let voyage = effect("com.MusicalEntropy.VoyageVoyagex1", dir.file("p.pst"), "p");
    let made = state(&voyage, Some(b"template"), |_| panic!("asked the player"));
    assert!(made.is_err());
    assert!(built::check("audio.mikey.Shu", &shu.source).is_ok());
    let missing = effect("audio.mikey.Shu", dir.file("Shu.clap"), "Missing");
    assert!(built::check("audio.mikey.Shu", &missing.source).is_err());
}

#[test]
fn effect_presets_without_a_template_are_asked_of_the_player() {
    let dir = Dir::new("effect-ask");
    let shu = effect("audio.mikey.Shu", dir.file("Shu.clap"), "Ether");
    let made = state(&shu, None, |_| Ok(vec![1]));
    assert_eq!(made.unwrap(), [1]);
}
