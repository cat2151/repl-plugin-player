"""Check that the Snippets window follows keybinds.toml in the real app.

Rewrites keybinds.toml so that snippet_run = F6 and snippet_close = F4 (always
restored on exit), runs the release executable with LOCALAPPDATA pointing at a
temporary directory holding one test-only user snippet, and sends keys with
keybd_event. The accessibility tree (UI Automation) is read to observe the result:
- Enter and Escape (the former keys) neither insert the snippet nor close the window
- F1 shows the new keys on the help page of the Snippets window
- F6 inserts the snippet into the editor and its value reaches the console
- F4 closes the window
The user's own files other than keybinds.toml are never touched.
"""
import ctypes
import ctypes.wintypes as w
import os
import subprocess
import tempfile
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target' / 'release' / 'repl-plugin-player.exe'
KEYBINDS = ROOT / 'keybinds.toml'
TITLE = 'REPL Plugin Player'
PROBE = 'zzkeybindprobe'
PROBE_CODE = '"keybind-probe-" + (40 + 2)'
PROBE_RESULT = 'keybind-probe-42'
user32 = ctypes.windll.user32
VK = {'Ctrl': 0x11, 'Shift': 0x10, 'Enter': 0x0D, 'Escape': 0x1B, 'P': 0x50,
      'F1': 0x70, 'F4': 0x73, 'F6': 0x75}
KEYEVENTF_KEYUP = 0x2
# Windows PowerShell 5.1, because pwsh lacks System.Windows.Automation.
SNAPSHOT = r'''
param([int]$ProcessId)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $ProcessId)
foreach ($e in $A::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)) {
  $v = ''
  try { $v = $e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value } catch {}
  $n = $e.Current.Name
  [Console]::Out.Write($n + [char]0x1f + $v + [char]0x1e)
}
'''


def catalog_texts(key):
    """The text in every locale, since the app follows the system's."""
    return {tomllib.loads(path.read_text(encoding='utf-8'))[key]
            for path in sorted((ROOT / 'locales').glob('*.toml'))}


def snapshot(pid):
    """(names, values) of every element of the process in the accessibility tree."""
    result = subprocess.run(['powershell.exe', '-NoProfile', '-Command',
                             f'& {{ {SNAPSHOT} }} -ProcessId {pid}'], capture_output=True)
    assert result.returncode == 0, result.stderr.decode('utf-8', 'replace')
    rows = [row.split('\x1f') for row in result.stdout.decode('utf-8').split('\x1e') if row]
    return {name for name, _ in rows}, [value for _, value in rows if value]


def wait_for(pid, condition, what, seconds=10.0):
    deadline = time.monotonic() + seconds
    while True:
        names, values = snapshot(pid)
        if condition(names, values):
            return names, values
        assert time.monotonic() < deadline, f'timed out: {what}'
        time.sleep(0.3)


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
    codes = [VK[key] if key in VK else ord(key.upper()) for key in keys]
    for code in codes:
        user32.keybd_event(code, 0, 0, 0)
        time.sleep(0.03)
    for code in reversed(codes):
        user32.keybd_event(code, 0, KEYEVENTF_KEYUP, 0)
        time.sleep(0.03)
    time.sleep(0.4)


def with_test_keys(text):
    for before, after in [('snippet_run = "Enter"', 'snippet_run = "F6"'),
                          ('snippet_close = "Escape"', 'snippet_close = "F4"')]:
        assert text.count(before) == 1, f'keybinds.toml has no {before}'
        text = text.replace(before, after)
    return text


def check(pid):
    ready = catalog_texts('m044')
    search = catalog_texts('m024')  # The filter's label: present while the window is open.
    window_open = lambda names, _: bool(search & names)
    inserted = lambda _, values: any(PROBE_CODE in value for value in values)

    wait_for(pid, lambda names, _: bool(ready & names), 'startup preparation', 180.0)
    tap('Ctrl', 'Shift', 'P')
    wait_for(pid, window_open, 'Ctrl+Shift+P opens the Snippets window')
    for letter in PROBE:
        tap(letter)
    wait_for(pid, lambda _, values: PROBE in values, 'the filter receives the typed name')

    for old in ['Enter', 'Escape']:
        tap(old)
        time.sleep(1.0)
        names, values = snapshot(pid)
        assert window_open(names, values), f'{old} closed the window'
        assert not inserted(names, values), f'{old} inserted the snippet'
        print(f'ok: {old} (the former key) does nothing')
        tap('Ctrl', 'Shift', 'P')  # The filter dropped focus on that key.

    tap('F1')
    names, _ = wait_for(pid, lambda names, _: {'F6', 'F4'} <= names,
                        'F1 shows the new keys on the Snippets page')
    assert not {'Enter', 'Escape'} & names, names
    print('ok: help shows ' + ', '.join(sorted({'F6', 'F4', '/', 'ArrowUp / ArrowDown'} & names)))
    tap('Escape')
    wait_for(pid, lambda names, _: 'F6' not in names, 'Escape closes the help')
    tap('Ctrl', 'Shift', 'P')

    tap('F6')
    wait_for(pid, lambda names, values: inserted(names, values)
             and any(PROBE_RESULT in value for value in values)
             and not window_open(names, values),
             'F6 inserts and runs the snippet, then the window closes')
    print('ok: F6 inserts the snippet and the console shows its value')

    tap('Ctrl', 'Shift', 'P')
    wait_for(pid, window_open, 'Ctrl+Shift+P reopens the Snippets window')
    tap('F4')
    wait_for(pid, lambda names, values: not window_open(names, values), 'F4 closes the window')
    print('ok: F4 closes the window')


def main():
    original = KEYBINDS.read_bytes()
    with tempfile.TemporaryDirectory() as root:
        snippets = Path(root) / 'repl-plugin-player' / 'snippets'
        snippets.mkdir(parents=True)
        (snippets / f'{PROBE}.js').write_text(PROBE_CODE + '\n', encoding='utf-8')
        process = None
        try:
            KEYBINDS.write_bytes(with_test_keys(original.decode('utf-8')).encode('utf-8'))
            process = subprocess.Popen([str(EXE)], env=dict(os.environ, LOCALAPPDATA=root))
            hwnd = None
            for _ in range(100):
                time.sleep(0.2)
                hwnd = find_window(process.pid)
                if hwnd:
                    break
            assert hwnd, 'window not found'
            # The exe read keybinds.toml at startup; restore it before the slow part.
            KEYBINDS.write_bytes(original)
            time.sleep(2.0)
            user32.SetForegroundWindow(hwnd)
            time.sleep(0.5)
            assert user32.GetForegroundWindow() == hwnd, 'window is not in the foreground'
            check(process.pid)
        finally:
            KEYBINDS.write_bytes(original)
            if process:
                subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)],
                               capture_output=True)
                process.wait()
            time.sleep(0.5)  # Let the temporary directory's files be released.
    print('PASS the Snippets window runs and closes with the keys of keybinds.toml')


if __name__ == '__main__':
    main()
