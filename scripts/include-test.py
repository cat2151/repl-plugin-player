"""Exercise include on real QuickJS, source and standalone embedded catalogs.

Run after cargo build --release. Uses normal Ready without starting playback.
Test scripts, catalogs and user data are isolated outside the repository.
"""
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    harness = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))
    session_type = harness['Session']
    with tempfile.TemporaryDirectory(prefix='repl-include-') as temporary:
        base = Path(temporary)
        user = base / 'user/repl-plugin-player'
        scripts = user / 'snippets/lib'
        scripts.mkdir(parents=True)
        cache = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/uapmd/plugin-list-cache.json'
        if cache.is_file():
            (user / 'uapmd').mkdir()
            shutil.copy2(cache, user / 'uapmd' / cache.name)
        standalone = base / 'app/repl-plugin-player.exe'
        standalone.parent.mkdir()
        shutil.copy2(ROOT / 'target/release/repl-plugin-player.exe', standalone)
        env = dict(os.environ, LOCALAPPDATA=str(base / 'user'))

        def launch(exe):
            def start(*args, **kwargs):
                return subprocess.Popen([str(exe), '--lang', 'en', *args], cwd=base,
                                        env=env, text=True, encoding='utf-8',
                                        creationflags=subprocess.CREATE_NO_WINDOW, **kwargs)
            session_type.__init__.__globals__['launch'] = start
            return session_type()

        for exe in (ROOT / 'target/release/repl-plugin-player.exe', standalone):
            session = launch(exe)
            try:
                # Function results stay in JS, and loading performs no audio mutations.
                assert session.evaluate("""(() => {
                    var count = __replEvents.length;
                    var demo = include('builtin/lib/tutorial.js');
                    var loop = include('builtin/lib/loop.js');
                    return count === __replEvents.length && typeof demo.reset === 'function'
                        && typeof demo.replaceMml === 'function' && typeof demo.init === 'function'
                        && typeof demo.play === 'function' && typeof loop.publishLoopParts === 'function';
                })()""") is True
                child = scripts / '猫.js'
                child.write_text("({value:'first', call:()=>42, scope:typeof callerOnly})", encoding='utf-8')
                (scripts / 'parent.js').write_text("include('user/lib/猫.js')", encoding='utf-8')
                code = "(() => { var callerOnly = 123; var lib = include('user/lib/parent.js'); return [lib.value,lib.call(),lib.scope]; })()"
                assert session.evaluate(code) == ['first', 42, 'undefined']
                child.write_text("({value:'edited', call:()=>43, scope:typeof callerOnly})", encoding='utf-8')
                assert session.evaluate(code) == ['edited', 43, 'undefined']
                # Repeated global script declarations and side effects have loadScript semantics.
                child.write_text("var includeCount = (globalThis.includeCount || 0) + 1; includeCount", encoding='utf-8')
                assert session.evaluate("include('user/lib/猫.js')") == 1
                assert session.evaluate("include('user/lib/猫.js')") == 2
                for contents in ('const = ;', "throw new Error('broken')", 'throw null'):
                    child.write_text(contents, encoding='utf-8')
                    error = session.request('eval', ok=False, code="include('user/lib/parent.js')")
                    assert 'user/lib/parent.js' in error and 'user/lib/猫.js' in error, error
                for name in ('builtin/lib/missing.js', 'user/../outside.js', 'other/lib/test.js'):
                    error = session.request('eval', ok=False, code=f'include({json.dumps(name)})')
                    assert name in error, error
                child.write_text('({recovered:true})', encoding='utf-8')
                assert session.evaluate("include('user/lib/parent.js')") == {'recovered': True}
                session.finish(failure=True)
            finally:
                session.kill()

        # An external builtin catalog overrides embedded files and is reread in-session.
        builtin = standalone.parent / 'snippets/builtin/lib'
        builtin.mkdir(parents=True)
        # Startup evaluates builtin/init.js, so an authoritative catalog carries it.
        shutil.copy2(ROOT / 'snippets/builtin/init.js', builtin.parent / 'init.js')
        shutil.copy2(ROOT / 'snippets/builtin/lib/plugins.js', builtin / 'plugins.js')
        probe = builtin / 'probe.js'
        probe.write_text('41', encoding='utf-8')
        session = launch(standalone)
        try:
            assert session.evaluate("include('builtin/lib/probe.js')") == 41
            probe.write_text('42', encoding='utf-8')
            assert session.evaluate("include('builtin/lib/probe.js')") == 42
            error = session.request('eval', ok=False, code="include('builtin/lib/tutorial.js')")
            assert 'builtin/lib/tutorial.js' in error, error
            session.finish(failure=True)
        finally:
            session.kill()
    print('PASS include: real QuickJS, nested functions, global scope, live edits, errors and recovery, foreign CWD, embedded and external catalogs')


if __name__ == '__main__':
    main()
