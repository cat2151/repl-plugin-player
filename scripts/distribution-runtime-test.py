"""Installed-Surge check of the extracted static package, outside the project."""
import argparse
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile
import zipfile


def session(exe, cwd, requests, failure=False, language="en"):
    result = subprocess.run([str(exe), '--lang', language, '--automation'], cwd=cwd,
                            input=''.join(json.dumps(v) + '\n' for v in requests).encode(),
                            capture_output=True, timeout=180, creationflags=subprocess.CREATE_NO_WINDOW)
    events = [json.loads(line) for line in result.stdout.decode('utf-8').splitlines()]
    assert (result.returncode != 0) == failure, (result.returncode, events, result.stderr)
    assert events[0]['event'] == 'ready', events
    closed = [event for event in events if event.get('event') == 'closed']
    assert len(closed) == 1 and closed[0]['ok'], events
    return {event['id']: event for event in events if 'id' in event}


def main(package):
    with tempfile.TemporaryDirectory(prefix='repl-distribution-') as temporary:
        base = Path(temporary)
        app = base / 'app'
        with zipfile.ZipFile(package) as archive:
            archive.extractall(app)
        elsewhere = base / 'elsewhere'
        elsewhere.mkdir()
        exe = app / 'repl-plugin-player.exe'
        assert not (app / 'uapmd_host.dll').exists()
        assert not (app / 'locales').exists()
        session(exe, elsewhere, [{'id': 1, 'command': 'close'}], language='ja')
        user = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player'
        for lang in ('en', 'ja'):
            assert (user / 'locales' / f'{lang}.toml').is_file()
        host = (Path(__file__).resolve().parents[1] / 'build/out/uapmd_host.dll').read_bytes()
        caches = list((user / 'native').glob('*/uapmd_host.dll'))
        assert any(path.read_bytes() == host for path in caches), 'Embedded UAPMD host DLL was not extracted intact'

        replies = session(exe, elsewhere, [
            # Test-owned scripts: the embedded lib is reachable, and repl.ui is GUI-only.
            {'id': 1, 'command': 'eval', 'code': "uapmd.timeline.createEmptyMidiClip("
             "include('builtin/lib/tracks.js').idOf(0), 0, 480, 120)"},
            {'id': 2, 'command': 'eval', 'code': "({clips:uapmd.timeline.getClips(include('builtin/lib/tracks.js').idOf(0)).length,"
             "audio:uapmd.audio.isEngineEnabled()})"},
            {'id': 3, 'command': 'eval', 'code': "repl.ui.show({id:'package-probe', title:'probe', widgets:[]})"},
            {'id': 4, 'command': 'eval', 'code': "repl.ui.close('package-probe')"},
            {'id': 5, 'command': 'close'},
        ], failure=True)
        assert replies[1]['ok'], replies
        assert replies[2]['ok'] and replies[2]['result']['value']['clips'] == 1, replies[2]
        assert replies[2]['result']['value']['audio'] is False, replies[2]
        for identifier in (3, 4):
            assert not replies[identifier]['ok'] and 'normal GUI' in replies[identifier]['error'], replies[identifier]
        # An adjacent external builtin directory must override the embedded set,
        # proving that a packaged exe does not reach back into its build source.
        builtin = app / 'snippets/builtin'
        (builtin / 'lib').mkdir(parents=True)
        # Startup evaluates builtin/init.js, so an authoritative catalog carries it.
        source = Path(__file__).resolve().parents[1] / 'snippets/builtin'
        shutil.copy2(source / 'init.js', builtin / 'init.js')
        shutil.copy2(source / 'lib/plugins.js', builtin / 'lib/plugins.js')
        shutil.copy2(source / 'lib/tracks.js', builtin / 'lib/tracks.js')
        (builtin / 'package-probe.js').write_text('({packagedOverride:42})', encoding='utf-8')
        replies = session(exe, elsewhere, [
            {'id': 1, 'command': 'snippet', 'group': 'Built-in', 'name': 'package-probe'},
            {'id': 2, 'command': 'close'},
        ])
        assert replies[1]['result']['value'] == {'packagedOverride': 42}, replies
    print('PASS extracted static package: real normal Ready, embedded snippets, GUI-only snippet errors, adjacent override, foreign CWD and cleanup')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('package', type=Path)
    main(parser.parse_args().package)
