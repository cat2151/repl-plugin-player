"""Opt-in real-device regression with a supplied, read-only project archive."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("project", type=Path)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--workers", type=int, choices=range(33), help="Diagnostic override of the worker count")
    parser.add_argument("--frames", type=int, choices=(256, 512, 1024, 2048, 4096), help="Diagnostic device period override")
    args = parser.parse_args()
    if not args.project.is_file() or args.repeats < 1:
        parser.error("Supply an existing project archive and positive repeat count")
    archive = args.project.resolve()
    original = hashlib.sha256(archive.read_bytes()).digest()
    options = {"path": str(archive), "seconds": 15}
    if args.workers is not None:
        options["workers"] = args.workers
    if args.frames is not None:
        options["frames"] = args.frames
    requests = [
        {"id": "options", "command": "eval", "code": "globalThis.deviceBufferProbe = " + json.dumps(options)},
        {"id": "playback", "command": "script", "path": str(Path(__file__).with_name("playback.js"))},
        {"id": "close", "command": "close"},
    ]
    try:
        for repeat in range(args.repeats):
            result = subprocess.run(
                [str(ROOT / "target/release/repl-plugin-player.exe"), "--automation", "--timeout", "90"],
                cwd=ROOT, input="".join(json.dumps(r) + "\n" for r in requests),
                capture_output=True, text=True, encoding="utf-8", timeout=100,
            )
            replies = [json.loads(line) for line in result.stdout.splitlines()]
            if result.returncode or any(not r.get("ok") for r in replies):
                raise RuntimeError(f"Playback failed: {result.stdout}\n{result.stderr}")
            playback = next(r for r in replies if r.get("id") == "playback")
            ready = next(r for r in replies if r.get("event") == "ready")
            native_log = Path(ready["log"]).with_suffix(".native.stderr.log").read_text(encoding="utf-8")
            if "Audio deadline overrun:" in native_log:
                raise RuntimeError(f"Worker overrun: {ready['log']}")
            print(f"PASS {repeat + 1}: {json.dumps(playback['result']['value'])} | {ready['log']}", flush=True)
    finally:
        if hashlib.sha256(archive.read_bytes()).digest() != original:
            raise RuntimeError("Project archive changed during playback")


if __name__ == "__main__":
    main()
