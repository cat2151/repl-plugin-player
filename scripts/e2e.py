"""Real UAPMD/device/installed-Surge checks, one process and device owner at a time."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target/release/repl-plugin-player.exe"
LOGS = Path(os.environ["LOCALAPPDATA"]) / "repl-plugin-player/logs"
CASES = ["cold", "sample", "rerun", "stop", "close", "syntax", "missing",
         "close-preparing", "close-creating", "mutation-close",
         "syntax-sustain", "runtime-error", "stop-run", "timeline", "timeline-view", "note-monitor", "scope", "partial-run", "snippets", "tutorials", "demo-play", "mml-clip", "chord-clip", "script-ui", "loop", "loop-update"]


def main():
    for case in sys.argv[1:] or CASES:
        existing = set(LOGS.glob("*.jsonl"))
        with subprocess.Popen([str(EXE), "--lang", "en", "--e2e", case], cwd=ROOT,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              creationflags=subprocess.CREATE_NO_WINDOW) as child:
            try:
                stdout, stderr = child.communicate(timeout=180)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
                raise RuntimeError(f"{case}: process {child.pid} timed out")
            paths = [path for path in LOGS.glob(f"*-{child.pid}.jsonl") if path not in existing]
            if child.returncode or len(paths) != 1:
                raise RuntimeError(f"{case}: exit {child.returncode}, logs {paths}; "
                                   f"{stdout[-1000:]!r} {stderr[-1000:]!r}")
            events = [json.loads(line) for line in paths[0].read_text(encoding="utf-8").splitlines()]
            assert any(e["event"] == "e2e_pass" and e["details"]["case"] == case for e in events), paths[0]
            assert any(e["event"] == "cleanup" and e["details"]["result"]["ok"] for e in events), paths[0]
            calls = [e for e in events if e["event"] == "api"]
            note_on = [e for e in calls if e["details"]["call"]["api"] == "uapmd.sequencer.sendNoteOn"]
            if case in ("syntax", "missing", "close-preparing", "close-creating", "mutation-close", "timeline", "timeline-view", "partial-run", "snippets", "mml-clip", "chord-clip", "script-ui", "loop", "loop-update"):
                assert not note_on, f"{case}: unexpected NoteOn"
            elif case in ("tutorials", "demo-play"):
                pass  # Built-in snippets may play notes however the user writes them.
            elif case == "stop-run":
                assert len(note_on) == 2, paths[0]
            elif case == "note-monitor":
                assert len(note_on) == 5, paths[0]
            elif case == "scope":
                assert len(note_on) == 3, paths[0]
                assert [e["details"]["note"] for e in events if e["event"] == "scope_verified"] == [60, 72, 67], paths[0]
            else:
                assert len(note_on) == 1, paths[0]
            # Run/error must never inject audio calls. Explicit test scripts
            # and user Stop/exit are the only post-Ready audio mutations.
            assert not any(e["details"]["reason"].startswith(("timer:", "before Run:", "script error"))
                           for e in calls), paths[0]
            if case in ("cold", "sample", "rerun", "syntax-sustain", "runtime-error", "stop-run"):
                explicit_off = [e for e in calls
                                if e["details"]["call"]["api"] == "uapmd.sequencer.sendNoteOff"
                                and e["details"]["reason"] == "Run: whole editor"]
                assert len(explicit_off) == 1, paths[0]
                delay = explicit_off[0]["monotonic_ms"] - note_on[-1]["monotonic_ms"]
                assert delay >= 1400, (paths[0], delay)
                first_run = next(e for e in events if e["event"] == "run_input")
                implicit_disable = [e for e in calls
                                    if e["monotonic_ms"] >= first_run["monotonic_ms"]
                                    and e["details"]["call"]["api"] == "uapmd.audio.setEngineEnabled"
                                    and e["details"]["call"]["args"] == [False]
                                    and e["details"]["reason"] not in
                                    ("E2E Stop before new Run", "E2E complete", "normal exit")]
                assert not implicit_disable, paths[0]
            summary = next((e["details"] for e in events if e["event"] == "signal_check"), {})
            print(f"PASS {case}: {json.dumps(summary)} | {paths[0]}", flush=True)


if __name__ == "__main__":
    main()
