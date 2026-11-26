"""Smoke editable prototypes twice, switch between them, then test native timing.

Use --smoke-only when the independent E2E cases have already run.

Uses the normal app log directory outside the repository. No snippet music is an
expectation; tests/e2e/live-sequence.js owns the timing/pitch fixtures.
"""
from pathlib import Path
import argparse
import runpy
import shutil
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--smoke-only', action='store_true')
    args = parser.parse_args()
    session_type = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    session = session_type()
    try:
        for _ in range(2):
            for name in ('try-evolving-sequencer',):
                session.request('snippet', group='Built-in', name=name)
                time.sleep(2.5)  # The idle CLI must service JS updates without another Run.
        session.request('snippet', group='Built-in', name='try-end-evolve')
        session.request('snippet', group='Built-in', name='try-end-evolve')
        session.request('snippet', group='Built-in', name='try-play')
        events = session.finish()
        assert not any(e['event'] == 'periodic_script_error' for e in events), session.log
        print('PASS editable evolving sequencer: repeated execution')
    finally:
        session.kill()
    # A copied exe must deploy the Rust engine, not depend on target/release.
    launch_globals = session_type.__init__.__globals__
    original_exe = launch_globals['EXE']
    with tempfile.TemporaryDirectory(prefix='repl-sequence-deploy-') as temporary:
        copied = Path(temporary) / original_exe.name
        shutil.copy(original_exe, copied)
        # Random catalog sounds use the player's existing headless state restorer.
        player = next((p for p in (
            original_exe.with_name('cat-plugin-player.exe'),
            ROOT.parent / 'cat-plugin-player/target/release/cat-plugin-player.exe',
            Path(shutil.which('cat-plugin-player.exe') or '')
        ) if p.is_file()), None)
        assert player is not None, 'cat-plugin-player is required for catalog patch restoration'
        shutil.copy(player, copied.with_name('cat-plugin-player.exe'))
        launch_globals['EXE'] = copied
        try:
            session = session_type()
            try:
                for name in ('try-evolving-sequencer',):
                    session.request('snippet', group='Built-in', name=name)
                    time.sleep(2.5)
                session.request('snippet', group='Built-in', name='try-end-evolve')
                events = session.finish()
                assert not any(e['event'] == 'periodic_script_error' for e in events), session.log
            finally:
                session.kill()
        finally:
            launch_globals['EXE'] = original_exe
    print('PASS standalone exe deploys Rust evolve.dll and services periodic JS')
    if args.smoke_only:
        return
    import subprocess
    import sys
    subprocess.run([sys.executable, str(ROOT / 'scripts/e2e.py'), 'live-sequence', 'evolve'],
                   cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
