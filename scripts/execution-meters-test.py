"""Verify timing telemetry for the load meter on the real engine."""
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

        run('setup')
        time.sleep(.15)
        observation = run('observe')
        assert {'deadlineMisses', 'droppedTimings'} <= observation.keys(), observation
        assert any(t['stage'] == 5 and t['frames'] > 0 and t['sampleRate'] > 0
                   for t in observation['processingTimings']), observation
        run('stop')
        session.finish()
        print('PASS execution meters: timings')
    finally:
        session.kill()


if __name__ == '__main__':
    main()
