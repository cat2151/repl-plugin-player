"""Check that config.toml's [window] sets the real window's position and size.

Runs the release executable with LOCALAPPDATA pointing at a temporary directory
that holds only config.toml, reads the window rectangle through Win32 and kills
the process tree. The user's own config.toml is never read or written.
"""
import ctypes
import ctypes.wintypes as w
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target' / 'release' / 'repl-plugin-player.exe'
TITLE = 'REPL Plugin Player'
user32 = ctypes.windll.user32


def find_window(pid):
    found = []

    @ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    def visit(hwnd, _):
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        title = ctypes.create_unicode_buffer(256)
        user32.GetWindowTextW(hwnd, title, 256)
        if owner.value == pid and title.value == TITLE and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    user32.EnumWindows(visit, 0)
    return found[0] if found else None


def measure(config):
    """Returns (outer x, outer y, client width, client height) in points."""
    with tempfile.TemporaryDirectory() as root:
        directory = Path(root) / 'repl-plugin-player'
        directory.mkdir()
        (directory / 'config.toml').write_text(config, encoding='utf-8')
        process = subprocess.Popen([str(EXE)], env=dict(os.environ, LOCALAPPDATA=root))
        try:
            hwnd = None
            for _ in range(100):
                time.sleep(0.2)
                hwnd = find_window(process.pid)
                if hwnd:
                    break
            assert hwnd, 'window not found'
            time.sleep(1.0)
            scale = user32.GetDpiForWindow(hwnd) / 96
            outer, client = w.RECT(), w.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(outer))
            user32.GetClientRect(hwnd, ctypes.byref(client))
            return tuple(round(v / scale) for v in (outer.left, outer.top, client.right, client.bottom))
        finally:
            subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)], capture_output=True)
            process.wait()
            time.sleep(0.5)  # Let the temporary directory's files be released.


def main():
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # Per-monitor v2.
    cases = [
        ('[window]\nx = 200\ny = 150\nwidth = 900\nheight = 600\n', (200, 150, 900, 600)),
        ('[window]\nx = 300\ny = 120\nwidth = -5\nheight = 700\n', (300, 120, 1250, 700)),
    ]
    for config, expected in cases:
        actual = measure(config)
        print(f'{config!r}: {actual}')
        assert actual == expected, f'expected {expected}'
    print('PASS window position and size follow config.toml')


if __name__ == '__main__':
    main()
