use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// Builds `build/out/repl_bridge.dll` with the C/C++ runtime that this Rust target links
/// (`crt-static` → /MT), so `cargo build` alone builds the native bridge (ADR 0029).
/// Configures only without a cache or on a runtime change: an explicit configure rewrites
/// the generated UAPMD patches and relinks the bridge (about 2 min), and `cmake --build`
/// reconfigures by itself when a patch's inputs change (`CMAKE_CONFIGURE_DEPENDS`).
/// The DLL is not watched: this script writes it, so Cargo would rerun the script and
/// recompile the exe once more after every bridge rebuild.
fn bridge(root: &Path) {
    for path in [
        "native",
        "patches/uapmd",
        "../uapmd/source",
        "../uapmd/external",
        "../uapmd/CMakeLists.txt",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    let mut runtime = String::from("MultiThreaded$<$<CONFIG:Debug>:Debug>");
    if !features.split(',').any(|feature| feature == "crt-static") {
        runtime.push_str("DLL");
    }
    let cache = fs::read_to_string(root.join("build/CMakeCache.txt")).unwrap_or_default();
    let configured = cache.lines().any(|line| {
        line.split_once('=').is_some_and(|(key, value)| {
            key.starts_with("REPL_MSVC_RUNTIME_LIBRARY:") && value == runtime
        })
    });
    if !configured {
        cmake(
            root,
            &[
                "-S",
                "native",
                "-B",
                "build",
                "-G",
                "Visual Studio 17 2022",
                "-DBUILD_SHARED_LIBS=OFF",
                "-DCPM_SOURCE_CACHE=OFF",
                &format!("-DREPL_MSVC_RUNTIME_LIBRARY={runtime}"),
            ],
        );
    }
    cmake(
        root,
        &[
            "--build",
            "build",
            "--config",
            "Release",
            "--target",
            "repl_bridge",
            "--parallel",
            "6",
        ],
    );
}

fn cmake(root: &Path, args: &[&str]) {
    // Cargo reads a build script's stdout as `cargo:` directives, so CMake writes to stderr.
    let status = Command::new("cmake")
        .args(args)
        .current_dir(root)
        .stdout(io::stderr())
        .status()
        .expect("Run cmake");
    assert!(status.success(), "cmake {} failed", args.join(" "));
}

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
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    bridge(&root);
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
