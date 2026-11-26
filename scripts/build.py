"""Build the release exe that runs outside the repository, with the extension DLLs embedded.

Usage: python scripts/build.py

`cargo build --release` alone builds everything that runs inside the repository, the
native bridge included (build.rs); this script adds only the embedding (ADR 0029).
"""

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
        print(f"Build failed: {error.cmd}", file=sys.stderr)
        sys.exit(error.returncode)
    except OSError as error:
        sys.exit(str(error))
