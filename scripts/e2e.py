"""Real UAPMD/device/installed-Surge checks, one process and device owner at a time."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target/release/repl-plugin-player.exe"
LOGS = Path(os.environ["LOCALAPPDATA"]) / "repl-plugin-player/logs"
CASES = ["evolve-host", "evolve-protocol", "evolve", "evolve-timbre", "live-sequence", "cold", "sample", "rerun", "stop", "close", "syntax", "missing",
         "close-preparing", "close-creating", "mutation-close", "scope-load", "scope-master",
         "syntax-sustain", "runtime-error", "stop-run", "timeline", "timeline-view", "piano-roll-cut-paste", "note-monitor", "scope", "partial-run", "snippets", "tutorials", "demo-play", "mml-clip", "chord-clip", "script-ui", "loop", "loop-update", "loop-replaces-clips", "patch-use", "patch-timbre", "patch-rescan", "patch-catalog-reload", "patch-browse", "state-load", "track-use", "reset-first-note", "dock", "dock-script-ui"]


def catalog_stalls(log):
    """Stalls in which the control thread ran the patch catalog itself.

    A sampled stack inside `patch_catalog` counts unless it waits on the lock: waiting for
    the startup preload to finish is not the catalog being read on the control thread.
    """
    frames = log.with_suffix(".frames.jsonl")
    if not frames.exists():
        return 0, []
    stalls = [json.loads(line) for line in frames.read_text(encoding="utf-8").splitlines()]
    bad = [{"ms": s["ms"], "samples": stack["count"], "innermost": stack["frames"][:12]}
           for s in stalls for stack in s.get("stacks", [])
           if any("patch_catalog" in f for f in stack["frames"])
           and not any("lock_contended" in f for f in stack["frames"])]
    return len(stalls), bad


PATCH_STALL_MS = 150


def cell_stalls(log, cell):
    """Frame-monitor stalls of `log` overlapping the cell's wall-clock window, in ms."""
    frames = log.with_suffix(".frames.jsonl")
    if not frames.exists():
        return []
    stalls = [json.loads(line) for line in frames.read_text(encoding="utf-8").splitlines()]
    return [round(s["ms"]) for s in stalls
            if s["endMs"] >= cell["startMs"] and s["startMs"] <= cell["endMs"]]


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
            paths = [path for path in LOGS.glob(f"*_{child.pid}.jsonl") if path not in existing]
            if child.returncode or len(paths) != 1:
                raise RuntimeError(f"{case}: exit {child.returncode}, logs {paths}; "
                                   f"{stdout[-1000:]!r} {stderr[-1000:]!r}")
            events = [json.loads(line) for line in paths[0].read_text(encoding="utf-8").splitlines()]
            assert any(e["event"] == "e2e_pass" and e["details"]["case"] == case for e in events), paths[0]
            assert any(e["event"] == "cleanup" and e["details"]["result"]["ok"] for e in events), paths[0]
            calls = [e for e in events if e["event"] == "api"]
            note_on = [e for e in calls if e["details"]["call"]["api"] == "uapmd.sequencer.sendNoteOn"]
            if case in ("scope-master", "evolve-host", "evolve-mode", "evolve-protocol", "evolve-timbre", "live-sequence", "syntax", "missing", "close-preparing", "close-creating", "mutation-close", "timeline", "timeline-view", "piano-roll-cut-paste", "partial-run", "snippets", "mml-clip", "chord-clip", "script-ui", "loop", "loop-update", "loop-replaces-clips", "patch-use", "patch-rescan", "patch-catalog-reload", "patch-browse", "state-load", "track-use", "reset-first-note", "dock", "dock-script-ui"):
                assert not note_on, f"{case}: unexpected NoteOn"
            elif case in ("tutorials", "demo-play", "easy", "browser-change", "evolve"):
                pass  # Built-in snippets and lib playback may play notes however they are written.
            elif case == "stop-run":
                assert len(note_on) == 2, paths[0]
            elif case == "patch-timbre":
                assert len(note_on) == 27, paths[0]
            elif case == "note-monitor":
                assert len(note_on) == 7, paths[0]
            elif case == "scope":
                assert len(note_on) == 4, paths[0]
                assert [e["details"]["note"] for e in events if e["event"] == "scope_verified"] == [60, 72, 67, 64, 60], paths[0]
            elif case == "scope-load":
                assert not note_on, paths[0]
                assert [e["details"]["note"] for e in events if e["event"] == "scope_verified"] == [67, 69], paths[0]
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
            if case in ("mml-clip", "chord-clip"):
                generated = LOGS.parent / "generated-midi"
                found = [e["details"]["value"] for e in events if e["event"] == "e2e_cell"
                         and e["details"]["title"].startswith("generated paths")]
                assert len(found) == 1 and all(Path(p).parent == generated for p in found[0]), found
            stalls, catalog = catalog_stalls(paths[0])
            if catalog:
                raise RuntimeError(f"{case}: the control thread ran the patch catalog: {catalog}")
            cells = [e["details"]["value"] for e in events if e["event"] == "e2e_cell"]
            if case == "patch-rescan":
                for cell in cells:
                    if isinstance(cell, dict) and "startMs" in cell:
                        cell_stall = cell_stalls(paths[0], cell)
                        print(json.dumps({**cell, "stalls": cell_stall}), flush=True)
                        # Six Sines' own getState twice and stateLoad stay (about 70-100 ms);
                        # the parameter value walks cost about 3 s.
                        if cell["pluginId"] == "org.baconpaul.six-sines" and not cell["created"]:
                            assert max(cell_stall, default=0) < PATCH_STALL_MS, \
                                f"{case}: patch cell stalled the UI: {cell_stall} ms"
            summary = next((e["details"] for e in events if e["event"] == "signal_check"),
                           cells[-1] if cells else {})
            print(f"PASS {case}: {json.dumps(summary)} | stalls {stalls} | {paths[0]}", flush=True)


if __name__ == "__main__":
    main()
