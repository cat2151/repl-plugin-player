"""Check normal startup and evolve entry/exit using test-owned JS fixtures."""
from pathlib import Path
import runpy
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    session_type = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    started = time.perf_counter()
    session = session_type()
    try:
        print(f'Ready: {time.perf_counter()-started:.3f}s', flush=True)
        state = session.evaluate('({mode:!!globalThis.evolutionMode, pool:!!globalThis.evolutionPool})')
        assert not state['mode'] and not state['pool'], state
        session.evaluate((ROOT / 'tests/e2e/lib.js').read_text(encoding='utf-8'))
        for cell in (ROOT / 'tests/e2e/evolution-mode.js').read_text(encoding='utf-8').split('// %% ')[1:]:
            title, code = cell.split('\n', 1)
            session.evaluate(code)
            print(f'PASS {title}', flush=True)
        events = session.finish()
        assert not any(e['event'] == 'periodic_script_error' for e in events), session.log
    finally:
        session.kill()


if __name__ == '__main__':
    main()
