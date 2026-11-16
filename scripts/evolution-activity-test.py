"""Verify evolve activity expires on pause/device-off and exits with the mode."""
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
        session.evaluate('(' + cells['create'] + ')(' + json.dumps(dict(sounds=sounds,workers=6,evolve=True)) + ')')
        session.evaluate('(' + cells['load'] + ')(' + json.dumps(dict(slot=0,sound=sounds[0])) + ')')
        def until(probe):
            deadline = time.monotonic() + 10
            while not probe():
                if time.monotonic() >= deadline:
                    snapshot=session.evaluate(cells['signal'])
                    raise AssertionError(dict(
                        transition='Activity/priority transition timed out',
                        signal={k:snapshot[k] for k in ['evolutionBoostedThreads','engineEnabled','dispatcherPlaying']}))
                time.sleep(.025)
        status = lambda: session.evaluate(cells['status'])
        until(lambda: status()[0]['state'] == 2)
        assert not any(s['activity'] for s in status()), 'Unstarted stream used a CPU core'
        observe = lambda: session.evaluate(cells['signal'])
        assert observe()['evolutionBoostedThreads'] == 0
        session.evaluate(cells['start'])
        until(lambda: any(s['activity'] for s in status()))
        until(lambda: observe()['evolutionBoostedThreads'] >= 1)
        start = time.perf_counter()
        before = cpu(session.child)
        time.sleep(.6)
        active = 100*(cpu(session.child)-before)/(time.perf_counter()-start)
        for stop, restart in [('pause','resume'), ('disable audio','enable audio')]:
            session.evaluate(cells[stop])
            until(lambda: not any(s['activity'] for s in status()))
            if stop == 'pause':
                until(lambda: observe()['evolutionBoostedThreads'] == 0)
            start = time.perf_counter()
            before = cpu(session.child)
            time.sleep(.6)
            idle = 100*(cpu(session.child)-before)/(time.perf_counter()-start)
            assert idle < active*.6, (stop,active,idle)
            print(f'PASS {stop}: active={active:.1f}% idle={idle:.1f}% of one core',flush=True)
            session.evaluate(cells[restart])
            until(lambda: any(s['activity'] for s in status()))
        session.evaluate(cells['cleanup'])
        # A stopped device thread can retain its TLS guard until the next
        # callback. It restores priority before doing any normal render work.
        session.evaluate(cells['enable audio'])
        until(lambda: observe()['evolutionBoostedThreads'] == 0)
        session.evaluate(cells['disable audio'])
        events = session.finish()
        assert not any(e['event']=='periodic_script_error' for e in events), session.log
        print('PASS mode cleanup, activity-thread destruction and normal priority restoration',flush=True)
    finally:
        session.kill()


if __name__ == '__main__':
    main()
