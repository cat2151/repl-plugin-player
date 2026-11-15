"""Freeze, edit, unfreeze and restore in a fresh app without audible playback.

Uses isolated LOCALAPPDATA outside the repository; --loop adds audible playback.
For audible real-project UX
checks use patches/uapmd/device-buffer-size/check_playback.py --freeze <archive>.
"""
import argparse
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--loop', action='store_true', help='Also run audible frozen loop playback')
    args = parser.parse_args()
    session_type = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    cells = {}
    for cell in (ROOT / 'tests/e2e/freeze.js').read_text(encoding='utf-8').split('// %% ')[1:]:
        title, code = cell.split('\n', 1)
        cells[title.strip()] = code
    with tempfile.TemporaryDirectory(prefix='repl-freeze-') as temporary:
        local = Path(temporary)
        cache = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/uapmd/plugin-list-cache.json'
        if cache.is_file():
            destination = local / 'repl-plugin-player/uapmd'
            destination.mkdir(parents=True)
            shutil.copy2(cache, destination / cache.name)

        def launch(*args, **kwargs):
            return subprocess.Popen(
                [str(ROOT / 'target/release/repl-plugin-player.exe'), '--lang', 'en', *args],
                cwd=ROOT, env=dict(os.environ, LOCALAPPDATA=str(local)),
                text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW, **kwargs)

        session_type.__init__.__globals__['launch'] = launch
        archive = local / 'frozen.uapmdz'
        invalid = local / 'invalid.uapmd'
        invalid.write_bytes(b'not a project')
        frames = None
        for attempt in range(2):
            session = session_type()
            try:
                session.evaluate('globalThis.freezeProjectPath = ' + json.dumps(str(archive)))
                session.evaluate('globalThis.freezeInvalidProjectPath = ' + json.dumps(str(invalid)))
                if attempt == 0:
                    session.evaluate(cells['prepare'])
                    assert session.evaluate(cells['state'])['policy'] == 'off'
                    session.evaluate(cells['on'])
                    session.request('wait', code=cells['ready'])
                    session.evaluate(cells['display states'])
                    first = session.evaluate(cells['state'])
                    assert first['policy'] == 'on' and first['frozenAudio']['peak'] > 0, first
                    if args.loop:
                        print('zero-start loop:', session.evaluate(cells['loop playback']), flush=True)
                        session.evaluate(cells['nonzero loop range'])
                        print('nonzero fractional loop:', session.evaluate(cells['loop playback']), flush=True)
                    session.evaluate('globalThis.freezeMinimumFrames = ' + str(first['frozenAudio']['frames']))
                    session.evaluate(cells['edit'])
                    session.request('wait', code=cells['ready'])
                    edited = session.evaluate(cells['state'])
                    assert edited['frozenAudio'], edited
                    frames = edited['frozenAudio']['frames']
                    assert frames > first['frozenAudio']['frames'], (first, edited)
                    session.evaluate(cells['save'])
                    session.evaluate(cells['invalid load while rendering'])
                    session.request('wait', code=cells['ready'])
                    # Replacement must work both while a new cache is rendering
                    # and while an already completed cache is active.
                    for title in ('load while rendering', 'load'):
                        session.evaluate(cells[title])
                        session.request('wait', code=cells['ready'])
                        loaded = session.evaluate(cells['state'])
                        assert loaded['policy'] == 'on', loaded
                        assert loaded['frozenAudio']['frames'] == frames, loaded
                        assert loaded['frozenAudio']['peak'] > 0, loaded
                    session.evaluate(cells['off'])
                    live = session.evaluate(cells['state'])
                    assert live['policy'] == 'off' and live['runtimeState'] == 'live', live
                    assert live['frozenAudio'] is None, live
                    if args.loop:
                        session.evaluate(cells['automatic freeze playback'])
                        session.request('wait', code=cells['ready'])
                        assert not session.evaluate('uapmd.timeline.getState().isPlaying')
                        print('PASS: score end freezes without Stop', flush=True)
                        session.evaluate('uapmd.audio.setEngineEnabled(false)')
                        session.evaluate(cells['off'])
                    session.evaluate(cells['snippet smoke'])
                    # Transport-only check: keep the engine silent, then close;
                    # without audio callbacks the playhead cannot reach score end.
                    session.evaluate(cells['start transport'])
                    session.evaluate(cells['on'])
                    print('deferred freeze:', session.evaluate(cells['waiting for stop']), flush=True)
                    session.evaluate(cells['stop transport'])
                    session.evaluate(cells['off'])
                else:
                    session.evaluate(cells['load'])
                    session.request('wait', code=cells['ready'])
                    restored = session.evaluate(cells['state'])
                    assert restored['policy'] == 'on', restored
                    assert restored['frozenAudio']['frames'] == frames, restored
                    assert restored['frozenAudio']['peak'] > 0, restored
                    session.evaluate(cells['off'])
                session.finish()
            finally:
                session.kill()
    print('PASS: manual policy, cached audio, edit regeneration, load during/after freeze, '
          'failed load preserves freeze, off, snippet smoke, fresh-process restoration')


if __name__ == '__main__':
    main()
