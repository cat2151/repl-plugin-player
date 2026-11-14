"""Save an MML timeline, replace it through load, and reload in a fresh app.

Run after python scripts/build.py. Uses isolated LOCALAPPDATA outside the repo;
no playback or windows. Builtin snippets are checked only for execution errors.
"""
import json
import hashlib
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    session_type = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    cells = {}
    for cell in (ROOT / 'tests/e2e/project-save.js').read_text(encoding='utf-8').split('// %% ')[1:]:
        title, code = cell.split('\n', 1)
        cells[title.strip()] = code
    load_cells = {}
    for cell in (ROOT / 'tests/e2e/project-load.js').read_text(encoding='utf-8').split('// %% ')[1:]:
        title, code = cell.split('\n', 1)
        load_cells[title.strip()] = code
    with tempfile.TemporaryDirectory(prefix='repl-project-save-') as temporary:
        local = Path(temporary)
        user = local / 'repl-plugin-player'
        cache = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/uapmd/plugin-list-cache.json'
        if cache.is_file():
            (user / 'uapmd').mkdir(parents=True)
            shutil.copy2(cache, user / 'uapmd' / cache.name)

        def launch(*args, **kwargs):
            return subprocess.Popen(
                [str(ROOT / 'target/release/repl-plugin-player.exe'), '--lang', 'en', *args],
                cwd=ROOT, env=dict(os.environ, LOCALAPPDATA=str(local)),
                text=True, encoding='utf-8', creationflags=subprocess.CREATE_NO_WINDOW, **kwargs)

        session_type.__init__.__globals__['launch'] = launch
        path = user / 'projects/untitled.uapmdz'
        invalid_path = local / 'invalid.uapmdz'
        invalid_path.write_bytes(b'not a project')
        expected = None
        expected_loop = None
        for attempt in range(2):
            session = session_type()
            try:
                directory = Path(session.evaluate('repl.project.directory()'))
                assert directory == path.parent and directory.is_dir(), directory
                session.evaluate('globalThis.projectPath = ' + json.dumps(str(path)))
                session.evaluate('globalThis.invalidProjectPath = ' + json.dumps(str(invalid_path)))
                if attempt == 0:
                    session.evaluate(cells['snippet smoke'])
                    session.evaluate(cells['prepare'])
                    expected = session.evaluate(cells['snapshot'])
                    assert len(expected['clips']) == 1 and len(expected['clips'][0]['events']) >= 6, expected['clips']
                    assert expected['plugin'], 'No plugin state to preserve'
                    session.evaluate(cells['save'])
                    with zipfile.ZipFile(path) as archive:
                        assert archive.testzip() is None
                        assert 'project.uapmd' in archive.namelist(), archive.namelist()
                        assert any(name.startswith('clips/') and name.endswith('.midi2')
                                   for name in archive.namelist()), archive.namelist()
                        assert any(name.startswith('plugin_states/')
                                   for name in archive.namelist()), archive.namelist()
                    # Verify replacement of an existing destination creates a fresh archive.
                    path.write_bytes(b'old destination')
                    session.evaluate(cells['save'])
                    assert zipfile.is_zipfile(path)
                    assert session.evaluate(cells['invalid destination']) is True
                    assert session.evaluate(load_cells['invalid source']) is True
                    assert session.evaluate(cells['snapshot']) == expected
                    session.evaluate(load_cells['change'])
                    assert session.evaluate(cells['snapshot']) != expected
                    session.evaluate(load_cells['prepare outgoing loop'])
                    assert session.evaluate(load_cells['load']) == str(path)
                    cleared = session.evaluate(load_cells['outgoing loop cleared'])
                    assert cleared is True, cleared
                    assert session.evaluate(cells['snapshot']) == expected
                    session.evaluate(load_cells['load'])
                    assert session.evaluate(cells['snapshot']) == expected
                    session.evaluate(load_cells['snippet smoke'])
                    session.evaluate(load_cells['snippet smoke'])
                else:
                    session.evaluate(load_cells['load'])
                    actual = session.evaluate(cells['snapshot'])
                    differences = {}
                    for key in expected:
                        if actual[key] != expected[key]:
                            values = [expected[key], actual[key]]
                            differences[key] = ([{'length':len(value), 'sha256':hashlib.sha256(value.encode()).hexdigest()}
                                                 for value in values] if key == 'plugin' else values)
                    assert not differences, differences
                loop_path = user / 'projects/loop.uapmdz'
                session.evaluate('globalThis.projectPath = ' + json.dumps(str(loop_path)))
                if attempt == 0:
                    session.evaluate(cells['prepare saved loop'])
                    session.evaluate(cells['save'])
                else:
                    session.evaluate(load_cells['load'])
                    session.evaluate(cells['save'])
                with zipfile.ZipFile(loop_path) as archive:
                    loop_data = json.loads(archive.read('extensions/repl-loop/loop.json'))
                signature = {key: loop_data[key] for key in ('bpm', 'sampleRate', 'lengthSamples', 'parts')}
                assert len(signature['parts']) == 1 and len(signature['parts'][0]['events']) > 0, signature
                if attempt == 0:
                    expected_loop = signature
                    session.evaluate(load_cells['change'])
                    session.evaluate(load_cells['load'])
                    session.evaluate(cells['save'])
                    with zipfile.ZipFile(loop_path) as archive:
                        restored = json.loads(archive.read('extensions/repl-loop/loop.json'))
                    assert {key: restored[key] for key in signature} == expected_loop
                    session.evaluate(cells['play snippet smoke'])
                else:
                    assert signature == expected_loop, signature
                session.finish()
            finally:
                session.kill()
    print('PASS: archive, overwrite, load replacement/repetition, invalid sources preserve current project, '
          'snippet execution, fresh-process restoration')


if __name__ == '__main__':
    main()
