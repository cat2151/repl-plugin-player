"""A full scan brings back a CLAP bundle that the plugin list cache lacks.

Run after python scripts/build.py, with the app closed. Drops one installed CLAP
bundle (not Surge XT, which Ready needs) from the real cache, starts the real exe,
runs AppModel::performPluginScanning(false) through UAPMD's own JS entry, and checks
that the bundle is back in the host catalog and in the cache. The real cache is
restored afterwards, whatever happens.
"""
import json
import os
import runpy
import shutil
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
Session = runpy.run_path(str(ROOT / 'scripts/cli-test.py'), run_name='cli_test')['Session']
CACHE = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/uapmd/plugin-list-cache.json'
BUNDLES = """(() => { const r = []; for (let i = 0; i < uapmd.catalog.getCount(); i++) {
  const p = uapmd.catalog.getPluginAt(i); if (p.format === 'CLAP') r.push(p.bundlePath); }
  return r; })()"""


def scan(session):
    session.evaluate('__remidy_scan_tool_perform_scanning(); 0')
    deadline = time.time() + 120
    time.sleep(0.5)
    while session.evaluate('__remidy_sequencer_isScanning()'):
        assert time.time() < deadline, 'scan did not finish'
        time.sleep(0.2)


def main():
    cache = json.loads(CACHE.read_text(encoding='utf-8'))
    plugins = cache['plugins']
    dropped = next(p['bundle'] for p in plugins
                   if p['format'] == 'CLAP' and 'surge-xt' not in p['id'] and Path(p['bundle']).exists())
    with tempfile.TemporaryDirectory(prefix='repl-clap-catalog-') as temp:
        saved = Path(temp) / CACHE.name
        shutil.copy2(CACHE, saved)
        try:
            stale = dict(cache, plugins=[p for p in plugins if p['bundle'] != dropped])
            CACHE.write_text(json.dumps(stale, indent=2), encoding='utf-8')
            session = Session(240)
            try:
                assert dropped not in session.evaluate(BUNDLES), 'Ready did not start from the stale cache'
                scan(session)
                assert dropped in session.evaluate(BUNDLES), f'{dropped} is still missing from the catalog'
                session.finish()
            except Exception:
                session.kill()
                raise
            rewritten = json.loads(CACHE.read_text(encoding='utf-8'))['plugins']
            assert any(p['bundle'] == dropped for p in rewritten), f'{dropped} is still missing from the cache'
        finally:
            shutil.copy2(saved, CACHE)
    print(f'PASS clap-catalog-complete: {Path(dropped).name} restored by a full scan')


if __name__ == '__main__':
    main()
