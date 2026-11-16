"""Compare master scope CPU cost on a fixed, continuously playing evolve voice.

Uses the external hashed states prepared by evolution-fixed-test.py. Test setup
and musical input come from the test fixture, never from editable snippets.
"""
import hashlib
import json
import os
from pathlib import Path
import runpy
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    manifest = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/investigations/evolution-fixed/manifest.json'
    sounds = json.loads(manifest.read_text(encoding='utf-8'))
    for sound in sounds:
        assert hashlib.sha256(Path(sound['file']).read_bytes()).hexdigest() == sound['sha256']
    cells = dict(cell.split('\n', 1) for cell in
                 (ROOT / 'tests/e2e/evolution-fixed.js').read_text(encoding='utf-8').split('// %% ')[1:])
    Session = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    cpu = runpy.run_path(str(ROOT / 'scripts/evolution-fixed-test.py'))['cpu_seconds']
    session = Session()
    try:
        session.evaluate('(' + cells['create'] + ')(' + json.dumps(dict(sounds=sounds, workers=6, evolve=True)) + ')')
        session.evaluate('(' + cells['load'] + ')(' + json.dumps(dict(slot=0, sound=sounds[0])) + ')')
        deadline = time.monotonic() + 30
        while session.evaluate(cells['status'])[0]['state'] != 2:
            assert time.monotonic() < deadline, 'Voice preparation timeout'
            time.sleep(.025)
        session.evaluate(cells['start'])
        time.sleep(1)
        measurements = []
        for target in [-1, -2, -2, -1]:
            session.evaluate(f'repl.scope.setTarget({target})')
            time.sleep(.3)
            before = session.evaluate(cells['observe'])
            started, used = time.perf_counter(), cpu(session.child)
            time.sleep(5)
            after = session.evaluate(cells['observe'])
            elapsed = time.perf_counter() - started
            result = dict(target=target, cpuPercentOfOneCore=100*(cpu(session.child)-used)/elapsed,
                          blocks=after['signal']['blocks']-before['signal']['blocks'],
                          peak=after['signal']['peak'], finite=after['signal']['finite'])
            counters = [o['diagnostics']['timingCounters'] for o in [before, after]]
            result['deadlineMisses'] = counters[1]['deadlineMisses']-counters[0]['deadlineMisses']
            assert result['blocks'] > 0 and result['finite'] and result['peak'] > 0, result
            measurements.append(result)
            print(json.dumps(result), flush=True)
        off = sum(m['cpuPercentOfOneCore'] for m in measurements if m['target'] == -1)/2
        on = sum(m['cpuPercentOfOneCore'] for m in measurements if m['target'] == -2)/2
        print(f'Master scope additional CPU: {on-off:.2f} percentage points of one core', flush=True)
        session.evaluate(cells['cleanup'])
        session.finish()
    finally:
        session.kill()


if __name__ == '__main__':
    main()
