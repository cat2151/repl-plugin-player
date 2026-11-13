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
        assert not (base / 'user/repl-plugin-player/locales').exists(), 'repo release must read source catalogs'
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
        # Every exe inside the project reads the (here broken) source catalogs.
        for directory in [project, project / 'target/debug']:
            directory.mkdir(parents=True, exist_ok=True)
            shutil.copy2(EXE, directory / EXE.name)
            _, error = launch(directory / EXE.name, ROOT, '--help', env=env, ok=False)
            assert 'do not match this executable' in error
        assert not (base / 'user/repl-plugin-player/locales').exists(), 'project exes must not extract catalogs'
        # A standalone exe uses LocalAppData.
        layouts = [base / 'standalone']
        for directory in layouts:
            directory.mkdir(parents=True, exist_ok=True)
            shutil.copy2(EXE, directory / EXE.name)
            output, _ = launch(directory / EXE.name, ROOT, '--lang', 'ja', '--help', env=env)
            assert '表示言語' in output
        catalogs = base / 'user/repl-plugin-player/locales'
        defaults = {'ja': ja, 'en': en}

        def assert_first_launch():
            for lang, original in defaults.items():
                assert (catalogs / f'{lang}.toml').read_bytes() == b'', 'user files start empty'
                assert (catalogs / f'{lang}.default.toml').read_bytes() == original
            assert set(p.name for p in catalogs.iterdir()) == {
                'en.toml', 'ja.toml', 'en.default.toml', 'ja.default.toml'}

        assert_first_launch()
        standalone = layouts[-1] / EXE.name
        # A user file holds only the changed keys; the rest comes from the embedded catalog.
        edited = 'cli_help = ' + json.dumps('user edited --lang help', ensure_ascii=False) + '\n'
        (catalogs / 'en.toml').write_text(edited, encoding='utf-8')
        for _ in range(2):
            output, _ = launch(standalone, ROOT, '--lang', 'en', '--help', env=env)
            assert output == 'user edited --lang help'
        output, _ = launch(standalone, ROOT, '--lang', 'en', '--invalid', env=env, ok=False)
        assert 'error' in output, 'unchanged keys must come from embedded English'
        assert (catalogs / 'en.toml').read_text(encoding='utf-8') == edited
        # The reference copy is rewritten on every launch.
        (catalogs / 'en.default.toml').write_text('stale = "x"', encoding='utf-8')
        launch(standalone, ROOT, '--lang', 'en', '--help', env=env)
        assert (catalogs / 'en.default.toml').read_bytes() == en
        # Unknown keys and placeholder mismatches are dropped without stopping startup.
        stale = edited + 'removed_key = "x"\ninstall_elapsed = "{$wrong}"\n'
        (catalogs / 'en.toml').write_text(stale, encoding='utf-8')
        output, _ = launch(standalone, ROOT, '--lang', 'en', '--help', env=env)
        assert output == 'user edited --lang help'
        assert (catalogs / 'en.toml').read_text(encoding='utf-8') == stale, 'user files are not rewritten'
        # A deleted user file comes back empty; the other one is kept.
        (catalogs / 'ja.toml').unlink()
        launch(standalone, ROOT, '--lang', 'en', '--help', env=env)
        assert (catalogs / 'ja.toml').read_bytes() == b''
        assert (catalogs / 'en.toml').read_text(encoding='utf-8') == stale
        # Broken TOML stops startup and names the file.
        (catalogs / 'ja.toml').write_text('broken = [', encoding='utf-8')
        _, error = launch(standalone, ROOT, '--help', env=env, ok=False)
        assert str(catalogs / 'ja.toml') in error
        # Concurrent first launches publish complete files and leave no temporary files.
        shutil.rmtree(catalogs)
        children = [subprocess.Popen([str(standalone), '--lang', 'en', '--help'], cwd=ROOT,
                    env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    creationflags=subprocess.CREATE_NO_WINDOW) for _ in range(4)]
        for child in children:
            out, err = child.communicate(timeout=30)
            assert child.returncode == 0, (out, err)
        assert_first_launch()
    print('PASS real source-release/source-root/source-debug/embedded-standalone, foreign CWD, overlay/reference/dropped keys, corrupt catalogs and concurrent startup')


if __name__ == '__main__':
    main()
