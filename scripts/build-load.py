"""Run a command while recording the machine's build load, to tell its timings from builds.

    python -B scripts/build-load.py <samples.jsonl> -- <command> ...

Every 0.2 s a line {"wallMs", "machine", "build"} is written: the machine's CPU% and
the summed CPU% (100 = one core) of rustc, cargo, link, cl and MSBuild. At the end the
windows where `build` exceeded one core are printed as {"busy": [[startMs, endMs], ...]}
(Unix epoch ms, as the frame monitor writes); stalls and requests overlapping them are
not the measured program's.
"""
import json
from pathlib import Path
import subprocess
import sys
import threading
import time

import psutil

BUILD = {"rustc.exe", "cargo.exe", "link.exe", "cl.exe", "msbuild.exe"}
PERIOD = 0.2


def sample(out, stop):
    procs = {}
    psutil.cpu_percent(None)
    with out.open("w", encoding="utf-8") as file:
        while not stop.wait(PERIOD):
            build = 0.0
            for proc in psutil.process_iter(["name"]):
                if (proc.info["name"] or "").lower() not in BUILD:
                    continue
                known = procs.setdefault(proc.pid, proc)
                try:
                    build += known.cpu_percent(None)
                except psutil.Error:
                    pass
            file.write(json.dumps({"wallMs": time.time() * 1000, "machine": psutil.cpu_percent(None),
                                   "build": build}) + "\n")
            file.flush()


def busy(out):
    """Each busy sample covers the time since the previous sample."""
    windows = []
    previous = None
    for line in out.read_text(encoding="utf-8").splitlines():
        s = json.loads(line)
        if s["build"] > 100:
            start = previous if previous is not None else s["wallMs"] - PERIOD * 1000
            if windows and start <= windows[-1][1]:
                windows[-1][1] = s["wallMs"]
            else:
                windows.append([start, s["wallMs"]])
        previous = s["wallMs"]
    return windows


def main():
    out = Path(sys.argv[1])
    command = sys.argv[sys.argv.index("--") + 1:]
    stop = threading.Event()
    sampler = threading.Thread(target=sample, args=(out, stop))
    sampler.start()
    try:
        code = subprocess.call(command)
    finally:
        stop.set()
        sampler.join()
    print(json.dumps({"busy": busy(out)}), flush=True)
    sys.exit(code)


if __name__ == "__main__":
    main()
