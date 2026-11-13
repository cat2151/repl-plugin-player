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

/// `scripts/build.py` copies the workspace's `repl_keyboard.dll` here after a first build;
/// without it nothing is embedded and an exe outside the repository reports that.
fn keyboard() {
    println!("cargo:rerun-if-changed=build/out/repl_keyboard.dll");
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("build/out/repl_keyboard.dll");
    let source = if path.is_file() {
        let path_text = path.to_str().expect("UTF-8 keyboard DLL path");
        format!("const KEYBOARD: &[u8] = include_bytes!({path_text:?});\n")
    } else {
        "const KEYBOARD: &[u8] = &[];\n".to_owned()
    };
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("keyboard_dll.rs"),
        source,
    )
    .expect("Write embedded keyboard DLL");
}

fn main() {
    println!("cargo:rerun-if-changed=build/out/repl_bridge.dll");
    keyboard();
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
