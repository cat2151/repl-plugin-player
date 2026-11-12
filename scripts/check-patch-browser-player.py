"""Start the real resident cat-plugin-player through the patch browser link, then
check that it received the patch browser request and ends once this app's
process has ended.

Usage: python scripts/check-patch-browser-player.py [path to cat-plugin-player.exe]
(default: the sibling repo's release build)
"""
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEST = 'a_real_player_connects_and_stays_while_the_link_is_open'


def alive(pid):
    out = subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/NH', '/FO', 'CSV'],
                         capture_output=True, text=True).stdout
    return f'"{pid}"' in out


def main():
    default = ROOT.parent / 'cat-plugin-player/target/release/cat-plugin-player.exe'
    player = Path(sys.argv[1] if len(sys.argv) > 1 else default).resolve()
    if not player.is_file():
        sys.exit(f'FAIL: no player at {player}')
    env = dict(os.environ, CAT_PLUGIN_PLAYER_EXE=str(player))
    run = subprocess.run(['cargo', 'test', '--bin', 'repl-plugin-player', TEST, '--', '--nocapture'],
                         cwd=ROOT, env=env, capture_output=True, text=True,
                         encoding='utf-8', errors='replace')
    ended = time.time()
    if run.returncode != 0:
        sys.exit(f'FAIL: test failed\n{run.stdout[-3000:]}\n{run.stderr[-3000:]}')
    found = re.search(r'player pid (\d+)', run.stdout)
    if not found:
        sys.exit(f'FAIL: the test did not start a player\n{run.stdout[-3000:]}')
    pid = int(found.group(1))
    while alive(pid) and time.time() - ended < 15:
        time.sleep(0.1)
    log = (Path(tempfile.gettempdir()) / 'cat-resident-log' / 'player.log').read_text(encoding='utf-8')
    received = 'received Open(Patch { track: 0 })' in log
    remaining = alive(pid)
    print(f'pid={pid} received_open={received} alive={remaining} '
          f'waited={time.time() - ended:.2f}s')
    if not received or remaining:
        sys.exit(f'FAIL\n--- player.log ---\n{log[-2000:]}')
    print('PASS: the player showed the patch browser and ended with the app process')


if __name__ == '__main__':
    main()
