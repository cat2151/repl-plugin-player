"""Exercise release/source and embedded editable catalogs through real startup."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import json

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target/release/repl-plugin-player.exe'


def launch(exe, cwd, *args, env=None, ok=True):
    result = subprocess.run([str(exe), *args], cwd=cwd, env=env, capture_output=True,
                            timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
    assert (result.returncode == 0) == ok, (args, result.returncode, result.stdout, result.stderr)
    return result.stdout.decode('utf-8'), result.stderr.decode('utf-8')


def main():
    with tempfile.TemporaryDirectory(prefix='repl-localization-') as temporary:
        base = Path(temporary)
        env = dict(os.environ, LOCALAPPDATA=str(base / 'user'))
        for lang, expected in [('ja', '表示言語'), ('en', 'display language')]:
            output, _ = launch(EXE, base, '--help', '--lang', lang, env=env)
            assert expected in output
        assert not (base / 'user').exists(), 'repo release must read source catalogs'
        _, error = launch(EXE, base, '--lang', 'invalid', '--help', env=env, ok=False)
        assert 'ja or en' in error
        project = base / 'project'
        project.mkdir()
        (project / 'Cargo.toml').write_text('[package]\nname="repl-plugin-player"', encoding='utf-8')
        source = project / 'locales'
        shutil.copytree(ROOT / 'locales', source)
        release = project / 'target/release'
        release.mkdir(parents=True)
        exe = release / EXE.name
        shutil.copy2(EXE, exe)
        # A misleading adjacent catalog is ignored by the exact release layout.
        (release / 'locales').mkdir()
        launch(exe, base, '--lang', 'ja', '--help', env=env)
        en = (source / 'en.toml').read_bytes()
        ja = (source / 'ja.toml').read_bytes()
        (source / 'ja.toml').unlink()
        _, error = launch(exe, base, '--help', env=env, ok=False)
        assert str(source / 'ja.toml') in error
        (source / 'ja.toml').write_text('broken = [', encoding='utf-8')
        _, error = launch(exe, base, '--help', env=env, ok=False)
        assert 'ja.toml' in error
        (source / 'ja.toml').write_bytes(ja + b'unexpected_key = "bad"\n')
        _, error = launch(exe, base, '--help', env=env, ok=False)
        assert 'keys do not match' in error
        (source / 'ja.toml').write_bytes(ja.replace(b'{$seconds}', b'{$wrong}'))
        _, error = launch(exe, base, '--help', env=env, ok=False)
        assert 'placeholder mismatch' in error
        for lang, original in [('en', en), ('ja', ja)]:
            lines = original.decode('utf-8').splitlines()
            (source / f'{lang}.toml').write_text('\n'.join(line for line in lines if not line.startswith('m000 =')), encoding='utf-8')
        _, error = launch(exe, base, '--help', env=env, ok=False)
        assert 'do not match this executable' in error
        # Root, debug and standalone layouts all use LocalAppData, even with bad source TOMLs.
        layouts = [project, project / 'target/debug', base / 'standalone']
        for directory in layouts:
            directory.mkdir(parents=True, exist_ok=True)
            shutil.copy2(EXE, directory / EXE.name)
            output, _ = launch(directory / EXE.name, ROOT, '--lang', 'ja', '--help', env=env)
            assert '表示言語' in output
        catalogs = base / 'user/repl-plugin-player/locales'
        assert (catalogs / 'ja.toml').read_bytes() == ja
        assert (catalogs / 'en.toml').read_bytes() == en
        standalone = layouts[-1] / EXE.name
        # Existing edits persist across launches; added defaults are merged only in memory.
        edited = 'cli_help = ' + json.dumps('user edited --lang help', ensure_ascii=False) + '\n'
        (catalogs / 'en.toml').write_text(edited, encoding='utf-8')
        for _ in range(2):
            output, _ = launch(standalone, ROOT, '--lang', 'en', '--help', env=env)
            assert output == 'user edited --lang help'
        output, _ = launch(standalone, ROOT, '--lang', 'en', '--invalid', env=env, ok=False)
        assert 'error' in output, 'missing keys must be supplemented from embedded English'
        (catalogs / 'ja.toml').unlink()
        launch(standalone, ROOT, '--lang', 'en', '--help', env=env)
        assert (catalogs / 'ja.toml').read_bytes() == ja
        assert (catalogs / 'en.toml').read_text(encoding='utf-8') == edited
        (catalogs / 'ja.toml').write_text('m000 = "{$wrong}"', encoding='utf-8')
        _, error = launch(standalone, ROOT, '--lang', 'en', '--help', env=env, ok=False)
        assert str(catalogs / 'ja.toml') in error and 'placeholder mismatch' in error
        (catalogs / 'ja.toml').write_text('broken = [', encoding='utf-8')
        _, error = launch(standalone, ROOT, '--help', env=env, ok=False)
        assert str(catalogs / 'ja.toml') in error
        # Concurrent first launches publish complete catalogs and leave no temporary files.
        (catalogs / 'ja.toml').unlink()
        (catalogs / 'en.toml').unlink()
        children = [subprocess.Popen([str(standalone), '--lang', 'en', '--help'], cwd=ROOT,
                    env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    creationflags=subprocess.CREATE_NO_WINDOW) for _ in range(4)]
        for child in children:
            out, err = child.communicate(timeout=30)
            assert child.returncode == 0, (out, err)
        assert (catalogs / 'ja.toml').read_bytes() == ja
        assert (catalogs / 'en.toml').read_bytes() == en
        assert set(p.name for p in catalogs.iterdir()) == {'en.toml', 'ja.toml'}
    print('PASS real source-release/embedded-root/debug/standalone, foreign CWD, first/partial extraction, edits/restart/default merge, corrupt catalogs and concurrent startup')


if __name__ == '__main__':
    main()
