"""Check the patch browser entry of the real app: running a browse cell with
Ctrl+Enter starts one resident cat-plugin-player showing its patch browser; a
patch picked there is answered; Esc in the browser ends the browse, so running
the cell again asks the same player again; closing the app window leaves no
player behind.

Runs the release executable, replaces the document with the browse cell through
the clipboard and runs it until the pane is ready (it ignores the keys while
preparing), presses keys in the player's window, then closes the app window
with WM_CLOSE.
The app and any player are killed at the end whatever happened.

Usage: python scripts/check-patch-browser-entry.py
"""
import ctypes
import ctypes.wintypes as w
import re
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target' / 'release' / 'repl-plugin-player.exe'
TITLE = 'REPL Plugin Player'
PLAYER = 'cat-plugin-player.exe'
LOG = Path(tempfile.gettempdir()) / 'cat-resident-log' / 'player.log'
OPEN = 'received Open(Patch { track: 0 })'
user32 = ctypes.windll.user32
VK_CONTROL, VK_RETURN, VK_A, VK_V, VK_ESCAPE, VK_R = 0x11, 0x0D, 0x41, 0x56, 0x1B, 0x52
BROWSE = """{
    const track = 0;
    include('builtin/lib/patch.js').browse({track});
}
"""
KEYEVENTF_KEYUP = 0x2
MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP = 0x2, 0x4
# On the editor's text, in points from the window's top left. After a browse the
# keys sent here reach the editor only after a click there (a person's keys do
# without one).
EDITOR_TEXT = (200, 164)
WM_CLOSE = 0x0010


def find_window(pid, want=TITLE):
    """A visible window of `pid` titled `want`, or with any title when `want` is None."""
    found = []

    @ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
    def visit(hwnd, _):
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        title = ctypes.create_unicode_buffer(256)
        user32.GetWindowTextW(hwnd, title, 256)
        matches = title.value == want if want else bool(title.value)
        if owner.value == pid and matches and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    user32.EnumWindows(visit, 0)
    return found[0] if found else None


def players_of(pid):
    script = (f"Get-CimInstance Win32_Process -Filter \"ParentProcessId={pid} and Name='{PLAYER}'\""
              " | ForEach-Object { $_.ProcessId }")
    out = subprocess.run(['powershell', '-NoProfile', '-Command', script],
                         capture_output=True, text=True).stdout
    return [int(line) for line in out.split()]


def alive(pid):
    out = subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/NH', '/FO', 'CSV'],
                         capture_output=True, text=True).stdout
    return f'"{pid}"' in out


def logged(app_pid, line):
    """Times the player logged `line` since it connected to this app's link
    (a starting player rewrites the log)."""
    if not LOG.exists():
        return 0
    text = LOG.read_text(encoding='utf-8', errors='replace')
    connected = text.rfind(f'player connected repl-{app_pid}-')
    return 0 if connected < 0 else text[connected:].count(line)


def results(app_pid):
    """Patches the browser listed in its latest logged state; 0 while the catalog loads."""
    if not logged(app_pid, OPEN):
        return 0
    found = re.findall(r' results=(\d+) ', LOG.read_text(encoding='utf-8', errors='replace'))
    return int(found[-1]) if found else 0


def opens(app_pid):
    return logged(app_pid, OPEN)


def press(hwnd, *keys):
    """Holds `keys` in order, then releases them in reverse."""
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.2)
    assert user32.GetForegroundWindow() == hwnd, 'window is not in the foreground'
    for key in keys:
        user32.keybd_event(key, 0, 0, 0)
        time.sleep(0.03)
    for key in reversed(keys):
        user32.keybd_event(key, 0, KEYEVENTF_KEYUP, 0)
        time.sleep(0.03)


def run_browse(hwnd):
    """Replaces the document with the browse cell and runs it."""
    subprocess.run(['powershell', '-NoProfile', '-Command',
                    'Set-Clipboard -Value ([Console]::In.ReadToEnd())'],
                   input=BROWSE, text=True, check=True)
    # One key per frame: the editor takes Ctrl+Enter before it pastes.
    for key in (VK_A, VK_V, VK_RETURN):
        press(hwnd, VK_CONTROL, key)
        time.sleep(0.3)


def click_editor(hwnd):
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.2)
    rect = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    user32.SetCursorPos(rect.left + EDITOR_TEXT[0], rect.top + EDITOR_TEXT[1])
    time.sleep(0.2)
    user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
    time.sleep(0.05)
    user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)
    time.sleep(0.5)


def wait(condition, seconds, step=0.2):
    end = time.time() + seconds
    while time.time() < end:
        if condition():
            return True
        time.sleep(step)
    return condition()


def main():
    process = subprocess.Popen([str(EXE)])
    players = []
    try:
        hwnd = None
        assert wait(lambda: find_window(process.pid), 20), 'window not found'
        hwnd = find_window(process.pid)
        # The pane ignores the keys while preparing, so run until a player starts.
        deadline = time.time() + 90
        while not players and time.time() < deadline:
            run_browse(hwnd)
            time.sleep(1.5)
            players = players_of(process.pid)
        assert players, 'the browse cell started no player'
        assert len(players) == 1, f'more than one player: {players}'
        assert wait(lambda: opens(process.pid) >= 1, 15), 'the player got no Open'
        first = opens(process.pid)
        assert wait(lambda: find_window(players[0], None), 30), 'the player shows no window'
        browser = find_window(players[0], None)
        assert wait(lambda: results(process.pid) > 0, 60), 'the browser listed no patch'
        # R picks a random patch; the browse cell auditions it and answers.
        press(browser, VK_R)
        assert wait(lambda: logged(process.pid, 'received Applied(') >= 1, 60), 'the pick got no answer'
        print(f'picked a patch: answered ok={logged(process.pid, "received Applied(Ok(()))")}')
        # Esc ends the browse; only then does the app take input again.
        press(browser, VK_ESCAPE)
        click_editor(hwnd)
        run_browse(hwnd)
        assert wait(lambda: opens(process.pid) > first, 15), 'second browse sent no Open'
        again = players_of(process.pid)
        assert again == players, f'second browse changed the players: {players} -> {again}'
        print(f'player pid={players[0]} opens={opens(process.pid)}')
        # The app is busy browsing until the browser is hidden again.
        assert wait(lambda: find_window(players[0], None), 30), 'the player shows no window'
        press(find_window(players[0], None), VK_ESCAPE)
        user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
        assert wait(lambda: process.poll() is not None, 30), 'the app did not close'
        closed = time.time()
        assert wait(lambda: not alive(players[0]), 15), 'the player outlived the app'
        print(f'app exit={process.returncode} player gone after {time.time() - closed:.2f}s')
    finally:
        for pid in [process.pid, *players]:
            subprocess.run(['taskkill', '/T', '/F', '/PID', str(pid)], capture_output=True)
        process.wait()
    print('PASS a browse cell shows the patch browser of one player, which ends with the app')


if __name__ == '__main__':
    main()
