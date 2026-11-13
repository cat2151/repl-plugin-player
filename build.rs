use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn scripts(directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("Read builtin scripts") {
        let entry = entry.expect("Read builtin entry");
        let path = entry.path();
        if entry.file_type().expect("Read builtin file type").is_dir() {
            scripts(&path, paths);
        } else if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
        {
            paths.push(path);
        }
    }
}

/// Embeds every `<name>.dll` in `build/out/extensions`, where `scripts/build.py` copies
/// the extensions in this repository after a first build; an exe outside the repository
/// reports the names that are not embedded.
fn extensions() {
    println!("cargo:rerun-if-changed=build/out/extensions");
    let directory =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("build/out/extensions");
    let mut paths: Vec<PathBuf> = fs::read_dir(&directory)
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("Read extension DLL entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "dll"))
        .collect();
    paths.sort();
    let mut source = String::from("const EXTENSIONS: &[(&str, &[u8])] = &[\n");
    for path in paths {
        let name = path
            .file_stem()
            .unwrap()
            .to_str()
            .expect("UTF-8 extension name");
        let path_text = path.to_str().expect("UTF-8 extension DLL path");
        source.push_str(&format!("({name:?}, include_bytes!({path_text:?})),\n"));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("extension_dlls.rs"),
        source,
    )
    .expect("Write embedded extension DLLs");
}

fn main() {
    println!("cargo:rerun-if-changed=build/out/repl_bridge.dll");
    extensions();
    println!("cargo:rerun-if-changed=locales/ja.toml");
    println!("cargo:rerun-if-changed=locales/en.toml");
    let directory =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("snippets/builtin");
    println!("cargo:rerun-if-changed=snippets/builtin");
    let mut paths = Vec::new();
    scripts(&directory, &mut paths);
    paths.sort();
    assert!(
        paths
            .iter()
            .any(|path| path.file_name().unwrap() == "try-play.js"),
        "snippets/builtin/try-play.js is required"
    );
    let mut source = String::from("const EMBEDDED: &[(&str, &str)] = &[\n");
    for path in paths {
        let name = path
            .strip_prefix(&directory)
            .unwrap()
            .to_str()
            .expect("UTF-8 snippet name")
            .replace('\\', "/");
        let path_text = path.to_str().expect("UTF-8 snippet path");
        source.push_str(&format!("({name:?}, include_str!({path_text:?})),\n"));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("builtin_snippets.rs"),
        source,
    )
    .expect("Write embedded snippet catalog");
}
