"""Check that the settings overlay opens config.toml in the registered external editor.

Runs the release executable with LOCALAPPDATA pointing at a temporary directory
whose status.json registers a dummy editor (a .cmd that writes its argument to a
file). Keyboard only: Ctrl+, opens the overlay with the path field focused, Tab
moves to the "open config.toml" button and Space presses it. Then checks that the
editor received the full path of config.toml, that config.toml was created from
the template, and kills the process tree. The user's own files are never touched.
"""
import ctypes
import ctypes.wintypes as w
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target' / 'release' / 'repl-plugin-player.exe'
TITLE = 'REPL Plugin Player'
user32 = ctypes.windll.user32
VK_CONTROL, VK_TAB, VK_SPACE, VK_OEM_COMMA = 0x11, 0x09, 0x20, 0xBC
KEYEVENTF_KEYUP = 0x2
# Path field -> Save -> close -> open config.toml.
TABS_TO_OPEN_CONFIG = 3


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


def tap(*keys):
    for key in keys:
        user32.keybd_event(key, 0, 0, 0)
        time.sleep(0.03)
    for key in reversed(keys):
        user32.keybd_event(key, 0, KEYEVENTF_KEYUP, 0)
        time.sleep(0.03)
    time.sleep(0.3)


def main():
    with tempfile.TemporaryDirectory() as root:
        root = Path(root)
        directory = root / 'repl-plugin-player'
        directory.mkdir()
        received = root / 'received.txt'
        editor = root / 'editor.cmd'
        editor.write_text(f'@echo %~1> "{received}"\r\n', encoding='utf-8')
        (directory / 'status.json').write_text(json.dumps({'external_editor': str(editor)}),
                                               encoding='utf-8')
        config = directory / 'config.toml'
        process = subprocess.Popen([str(EXE)], env=dict(os.environ, LOCALAPPDATA=str(root)))
        try:
            hwnd = None
            for _ in range(100):
                time.sleep(0.2)
                hwnd = find_window(process.pid)
                if hwnd:
                    break
            assert hwnd, 'window not found'
            time.sleep(2.0)
            user32.SetForegroundWindow(hwnd)
            time.sleep(0.5)
            assert user32.GetForegroundWindow() == hwnd, 'window is not in the foreground'
            tap(VK_CONTROL, VK_OEM_COMMA)
            for _ in range(TABS_TO_OPEN_CONFIG):
                tap(VK_TAB)
            tap(VK_SPACE)
            for _ in range(100):
                if received.exists() and received.read_text(encoding='utf-8').endswith('\n'):
                    break
                time.sleep(0.1)
            assert received.exists(), 'the dummy editor did not run'
            actual = received.read_text(encoding='utf-8').strip()
            print(f'editor received: {actual}')
            assert Path(actual) == config, f'expected {config}'
            template = config.read_text(encoding='utf-8')
            assert template.startswith('# [window]'), template
        finally:
            subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)], capture_output=True)
            process.wait()
            time.sleep(0.5)  # Let the temporary directory's files be released.
    print('PASS Ctrl+, -> open config.toml passes its full path to the external editor')


if __name__ == '__main__':
    main()
