"""Loopback-record the default output device while the `first-note-dropout` E2E case plays,
then fail on any run of exact-zero samples inside a sounding note.

usage: python scripts/first-note-dropout-test.py [recording.npy]

A render-buffer underrun shows up only in what the device actually plays: Windows inserts
silence that no app-side observation (`repl.test.observe`, the device callback's output)
contains. The recording must hear nothing but this case, so other audio must be quiet.
Needs `soundcard` and `numpy`.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import warnings

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / "target/release/repl-plugin-player.exe"
LOGS = Path(os.environ["LOCALAPPDATA"]) / "repl-plugin-player/logs"
CASE = "first-note-dropout"
ATTEMPTS = 3
SR = 48000
RECORD_S = 20

EPS = 2.0 / 32768          # about 2 LSB of int16
MIN_RUN = 8                # samples
SILENCE_GAP_S = 0.100      # silence that separates plays
NEIGHBOR_S = 0.005         # window checked on each side of a zero run
NEIGHBOR_REL = 0.05        # neighbor peak must exceed this fraction of the play peak


def runs(mask):
    d = np.diff(np.concatenate([[0], mask.astype(np.int8), [0]]))
    return np.where(d == 1)[0], np.where(d == -1)[0]


def dropouts(x, sr):
    """Per play (sound after at least 100 ms of silence): its onset and the zero runs
    that have loud signal on both sides, as (start, length) in samples.

    The release tail, zero crossings and the gaps between plays are not reported."""
    env = np.abs(x).max(axis=1)
    qs, qe = runs(env <= EPS)
    gap = int(SILENCE_GAP_S * sr)
    long_q = [(s, e) for s, e in zip(qs, qe) if e - s >= gap or s == 0]
    plays = []
    for i, (_, e) in enumerate(long_q):
        if e < len(env):
            plays.append((e, long_q[i + 1][0] if i + 1 < len(long_q) else len(env)))
    nb = int(NEIGHBOR_S * sr)
    result = []
    for p0, p1 in plays:
        peak = env[p0:p1].max()
        found = []
        for s, e in zip(qs, qe):
            if s <= p0 or e >= p1 or e - s < MIN_RUN:
                continue
            before = env[max(p0, s - nb):s].max(initial=0.0)
            after = env[e:min(p1, e + nb)].max(initial=0.0)
            if before >= NEIGHBOR_REL * peak and after >= NEIGHBOR_REL * peak:
                found.append((int(s), int(e - s)))
        result.append({"onset": int(p0), "length": int(p1 - p0), "dropouts": found})
    return result


def record(seconds, out):
    import soundcard as sc
    warnings.filterwarnings("ignore", module="soundcard")
    speaker = sc.default_speaker()
    mic = sc.get_microphone(id=str(speaker.name), include_loopback=True)
    with mic.recorder(samplerate=SR, channels=2) as recorder:
        out.append(recorder.record(numframes=int(SR * seconds)))


def run_case():
    existing = set(LOGS.glob("*.jsonl"))
    with subprocess.Popen([str(EXE), "--lang", "en", "--e2e", CASE], cwd=ROOT,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          creationflags=subprocess.CREATE_NO_WINDOW) as child:
        try:
            stdout, stderr = child.communicate(timeout=RECORD_S)
        except subprocess.TimeoutExpired:
            child.kill()
            child.communicate()
            raise RuntimeError(f"{CASE}: process {child.pid} outlived the {RECORD_S} s recording")
        paths = [p for p in LOGS.glob(f"*_{child.pid}.jsonl") if p not in existing]
        if child.returncode or len(paths) != 1:
            raise RuntimeError(f"{CASE}: exit {child.returncode}, logs {paths}; "
                               f"{stdout[-1000:]!r} {stderr[-1000:]!r}")
        events = [json.loads(line) for line in paths[0].read_text(encoding="utf-8").splitlines()]
        assert any(e["event"] == "e2e_pass" and e["details"]["case"] == CASE for e in events), paths[0]
        return paths[0]


def main():
    recorded = []
    recorder = threading.Thread(target=record, args=(RECORD_S, recorded))
    recorder.start()
    try:
        log = run_case()
    finally:
        recorder.join()
    x = recorded[0].astype(np.float64)
    if len(sys.argv) > 1:
        np.save(sys.argv[1], x.astype(np.float32))
    plays = dropouts(x, SR)
    for n, play in enumerate(plays, 1):
        found = ", ".join(f"+{(s - play['onset']) / SR * 1000:.1f}ms x{ln}" for s, ln in play["dropouts"])
        print(f"play {n}: onset {play['onset'] / SR * 1000:.1f}ms, "
              f"{play['length'] / SR * 1000:.0f}ms, dropouts [{found}]", flush=True)
    assert len(plays) == ATTEMPTS, f"{len(plays)} plays recorded, {ATTEMPTS} expected (other audio, or none)"
    total = sum(len(p["dropouts"]) for p in plays)
    assert total == 0, f"{total} zero runs inside notes"
    print(f"PASS {CASE}: {ATTEMPTS} plays without zero runs | {log}", flush=True)


if __name__ == "__main__":
    main()
