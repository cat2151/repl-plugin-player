"""Assemble and check the static Windows x64 zip in an external work directory."""
import argparse
import os
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path
import crt_linkage
from build import BUNDLED, EMBEDDED

ROOT = Path(__file__).resolve().parents[1]


def package(output, executable, bridge):
    tool = crt_linkage.dumpbin()
    with tempfile.TemporaryDirectory(prefix='repl-package-') as temporary:
        stage = Path(temporary) / 'app'
        stage.mkdir()
        shutil.copy2(executable, stage / 'repl-plugin-player.exe')
        assert bridge.read_bytes() in executable.read_bytes(), 'Executable embeds a different bridge'
        extensions = [EMBEDDED / f'{name}.dll' for name in BUNDLED]
        assert sorted(EMBEDDED.iterdir()) == sorted(extensions), f'{EMBEDDED} differs from {BUNDLED}'
        for extension in extensions:
            assert extension.read_bytes() in executable.read_bytes(), f'Executable embeds a different {extension.name}'
            # The workspace build is the one `cargo test` checks against the exe's egui.
            assert extension.read_bytes() == (ROOT / 'target/release' / extension.name).read_bytes(), \
                f'Embedded {extension.name} differs from the one built with the exe'
        pending = [stage / 'repl-plugin-player.exe', bridge, *extensions]
        checked = set()
        imports = {}
        while pending:
            binary = pending.pop()
            if binary.name.lower() in checked:
                continue
            checked.add(binary.name.lower())
            dependencies = crt_linkage.dependents(tool, binary)
            imports[binary.name] = dependencies
            for name in dependencies:
                if crt_linkage.VC_RUNTIME.match(name) or crt_linkage.UCRT.match(name):
                    raise RuntimeError(f'{binary.name} imports dynamic CRT: {name}')
                if (stage / name).exists():
                    continue
                if name.lower().startswith(('api-ms-win-', 'ext-ms-win-')) or (Path(os.environ['SystemRoot']) / 'System32' / name).is_file():
                    continue
                candidates = list((ROOT / 'build').rglob(name))
                if not candidates:
                    raise RuntimeError(f'Unresolved dependency: {binary.name} -> {name}')
                # Reject ambiguous binaries rather than pick a random architecture/build.
                contents = {p.read_bytes() for p in candidates}
                if len(contents) != 1:
                    raise RuntimeError(f'Ambiguous dependency: {name}: {candidates}')
                shutil.copy2(candidates[0], stage / name)
                pending.append(stage / name)
        licenses = stage / 'licenses'
        # Include dependency notices from actual source checkouts/cache, not a guessed license.
        roots = [('uapmd', ROOT.parent / 'uapmd'), ('clap-mml-render-tui', ROOT.parent / 'clap-mml-render-tui'),
                 ('native-dependencies', ROOT / 'build/_deps')]
        metadata = subprocess.run(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=ROOT,
                                  capture_output=True, text=True, encoding="utf-8", check=True)
        import json
        for item in json.loads(metadata.stdout)['packages']:
            roots.append((f'rust/{item["name"]}-{item["version"]}', Path(item['manifest_path']).parent))
        for name, source in roots:
            for path in source.rglob('*'):
                if not path.is_file() or any(p in ('.git', 'target', 'build') for p in path.relative_to(source).parts):
                    continue
                if path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')):
                    destination = licenses / name / path.relative_to(source)
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(path, destination)
        assert any(licenses.rglob('*')), 'No third-party license files found'
        # Launch from outside the project: resources/DLLs cannot be borrowed from the repo.
        for lang in ('ja', 'en'):
            result = subprocess.run([str(stage / 'repl-plugin-player.exe'), '--lang', lang, '--help'],
                                    cwd=temporary, env=dict(os.environ, LOCALAPPDATA=str(Path(temporary) / 'user')),
                                    capture_output=True, timeout=30, check=True)
            assert result.stdout and b'--lang' in result.stdout, result.stderr
        subprocess.run([os.sys.executable, '-c',
                        'import ctypes,sys; d=ctypes.WinDLL(sys.argv[1]); assert d.repl_request and d.repl_set_script_api',
                        str(bridge)], cwd=temporary, check=True, timeout=30)
        output.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED, strict_timestamps=False) as archive:
            for path in stage.rglob('*'):
                if path.is_file():
                    archive.write(path, path.relative_to(stage))
        with zipfile.ZipFile(output) as archive:
            assert archive.testzip() is None
            names = set(archive.namelist())
            assert 'repl-plugin-player.exe' in names
            assert 'repl_bridge.dll' not in names
            assert not any(f'{name}.dll' in names for name in BUNDLED)
            assert not any(name.startswith('locales/') for name in names)
        print('PASS packaged imports:', imports)
        print('PASS isolated ja/en launch and zip:', output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--exe', type=Path, default=ROOT / 'target/release/repl-plugin-player.exe')
    parser.add_argument('--bridge', type=Path, default=ROOT / 'build/out/repl_bridge.dll')
    args = parser.parse_args()
    package(args.output.resolve(), args.exe.resolve(), args.bridge.resolve())
