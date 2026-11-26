"""Build the Windows release with matching Rust and native CRT linkage.

Usage: python scripts/build.py
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# The pane and audio-engine DLLs this repository holds; build.rs embeds these. Extensions
# cloned into `extensions/` from other repositories are not embedded.
BUNDLED = ["builtin", "evolve"]
EMBEDDED = ROOT / "build/out/extensions"


def main():
    # Same precedence as Cargo's environment flags. CI uses the target-specific
    # flag; no separate static-CRT switch is exposed.
    if "CARGO_ENCODED_RUSTFLAGS" in os.environ:
        flags = os.environ["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
    else:
        flags = os.environ.get(
            "RUSTFLAGS",
            os.environ.get("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS", ""),
        ).split()
    cfg = subprocess.run(
        ["rustc", "--print", "cfg", *flags],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    runtime = "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    if 'target_feature="crt-static"' not in cfg:
        runtime += "DLL"
    # Reconfigure the existing area when changing CRT, then rebuild all affected
    # native objects. Re-running without flags restores the ordinary /MD targets.
    subprocess.run(
        ["cmake", "-S", "native", "-B", "build", "-G", "Visual Studio 17 2022",
         "-DBUILD_SHARED_LIBS=OFF", "-DCPM_SOURCE_CACHE=OFF",
         f"-DREPL_MSVC_RUNTIME_LIBRARY={runtime}"],
        cwd=ROOT, check=True,
    )
    subprocess.run(
        ["cmake", "--build", "build", "--config", "Release", "--target",
         "repl_bridge", "--parallel", "6"],
        cwd=ROOT, check=True,
    )
    # Extension DLLs are built together with the exe, never with `-p <name>`: Cargo
    # unifies features only across the packages of one invocation, and a DLL whose
    # egui differs from the exe's aborts the process when it draws. The exe is then
    # rebuilt to embed the copied DLLs (build.rs watches the directory). `copy`, not
    # `copy2`: a preserved older mtime hides the change from rerun-if-changed.
    subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)
    if EMBEDDED.exists():
        shutil.rmtree(EMBEDDED)
    EMBEDDED.mkdir(parents=True)
    for name in BUNDLED:
        shutil.copy(ROOT / f"target/release/{name}.dll", EMBEDDED / f"{name}.dll")
    subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, file=sys.stderr, end="")
        print(f"Build failed: {error.cmd}", file=sys.stderr)
        sys.exit(error.returncode)
    except OSError as error:
        sys.exit(str(error))
