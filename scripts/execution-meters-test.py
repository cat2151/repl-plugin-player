"""Verify instrument input totals and timing telemetry on the real engine."""
import runpy
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    cells = dict(cell.split('\n', 1) for cell in
                 (ROOT / 'tests/e2e/execution-meters.js').read_text(encoding='utf-8').split('// %% ')[1:])
    session = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']()
    try:
        def run(title):
            return session.evaluate(cells[title])

        def count(expected):
            deadline = time.monotonic() + 5
            while True:
                observation = run('observe')
                if observation['noteCount'] == expected:
                    return observation
                assert time.monotonic() < deadline, (expected, observation)
                time.sleep(.025)

        run('setup')
        count(0)
        run('same pitch on two instruments')
        count(2)
        run('repeated note and effect input')
        count(3)
        run('release once')
        count(2)
        run('release second instrument')
        count(1)
        run('release remaining')
        count(0)
        time.sleep(.15)
        observation = run('observe')
        assert {'deadlineMisses', 'droppedTimings'} <= observation.keys(), observation
        assert any(t['stage'] == 5 and t['frames'] > 0 and t['sampleRate'] > 0
                   for t in observation['processingTimings']), observation
        run('same pitch on two instruments')
        count(2)
        run('stop')
        count(0)
        run('restart')
        count(0)
        run('stop')
        session.finish()
        print('PASS execution meters: instances, repeated pitch, effect exclusion, releases, Stop/restart, timings')
    finally:
        session.kill()


if __name__ == '__main__':
    main()
