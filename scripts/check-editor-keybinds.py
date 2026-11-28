"""Check that focus_editor, page_up / page_down and close_window follow keybinds.toml
in the real app.

Rewrites keybinds.toml so that focus_editor = F7, page_up = F8, page_down = F9 and
close_window = F6 (always restored on exit), runs the release executable with
LOCALAPPDATA pointing at a temporary directory holding one test-only user snippet,
and sends keys with keybd_event. The accessibility tree (UI Automation) is read to
observe the result:
- with focus on a button, Tab (the former key) leaves it there; F7 brings it back to
  the editor and letters typed next land in the editor
- in the editor, PageDown (the former key) does not move the caret; F9 moves it down
  and F8 back up
- F1 shows the new keys on the help page
- a repl.ui surface window stays on Escape (the former key) and closes on F6
The user's own files other than keybinds.toml are never touched.
"""
import importlib.util
import os
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    'snippet_keybinds', ROOT / 'scripts' / 'check-snippet-keybinds.py')
app = importlib.util.module_from_spec(spec)
spec.loader.exec_module(app)

SURFACE = 'zzkeybindsurface'
SURFACE_CODE = ("repl.ui.surface.show({id:'%s', width:240, height:120, "
                "regions:[{rect:[0, 0, 120, 120], color:[200, 60, 60]}]})" % SURFACE)
app.VK.update({'Tab': 0x09, 'Home': 0x24, 'PageUp': 0x21, 'PageDown': 0x22,
               'F7': 0x76, 'F8': 0x77, 'F9': 0x78})
# Windows PowerShell 5.1, because pwsh lacks System.Windows.Automation.
ELEMENTS = r'''
param([int]$ProcessId, [string]$Focus)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $ProcessId)
foreach ($e in $A::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)) {
  $c = $e.Current
  $type = $c.ControlType.ProgrammaticName
  if ($Focus -and $type -eq 'ControlType.Button' -and $c.Name -eq $Focus) { $e.SetFocus(); exit }
  $v = ''
  try { $v = $e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value } catch {}
  [Console]::Out.Write($type + [char]0x1f + $c.Name + [char]0x1f + $c.HasKeyboardFocus + [char]0x1f + $v + [char]0x1e)
}
'''


def run_elements(pid, focus=''):
    result = subprocess.run(['powershell.exe', '-NoProfile', '-Command',
                             f'& {{ {ELEMENTS} }} -ProcessId {pid} -Focus \'{focus}\''],
                            capture_output=True)
    assert result.returncode == 0, result.stderr.decode('utf-8', 'replace')
    return [row.split('\x1f') for row in result.stdout.decode('utf-8').split('\x1e') if row]


def editor(pid):
    """(has keyboard focus, text) of the code editor: the edit holding `MARK`."""
    edits = [(focus == 'True', value) for kind, _, focus, value in run_elements(pid)
             if kind == 'ControlType.Edit' and MARK in value]
    assert len(edits) == 1, edits
    return edits[0]


def type_text(text):
    for letter in text:
        app.tap(letter)


def focus_button(pid, names):
    for name in names:
        run_elements(pid, name)
    time.sleep(0.5)
    focused, _ = editor(pid)
    assert not focused, 'the editor kept focus'


MARK = 'qjm'  # Typed into the editor first, so the editor is the edit holding it.


def check(pid):
    app.wait_for(pid, lambda names, _: bool(app.catalog_texts('m044') & names),
                 'startup preparation', 180.0)
    app.tap('Ctrl', 'Home')
    type_text(MARK)
    focused, text = editor(pid)
    assert focused and text.startswith(MARK), 'the editor did not have focus at startup'

    scope = app.catalog_texts('pane_scope')
    focus_button(pid, scope)
    app.tap('Tab')
    type_text('qjt')
    focused, text = editor(pid)
    assert not focused and 'qjt' not in text, 'Tab (the former key) brought focus back'
    print('ok: Tab (the former key) leaves focus on the button')
    app.tap('F7')
    type_text('qjf')
    focused, text = editor(pid)
    assert focused and 'qjf' in text, 'F7 did not bring focus back to the editor'
    print('ok: F7 brings the caret back to the editor and typing continues there')

    app.tap('Ctrl', 'Home')
    app.tap('PageDown')
    type_text('qjv')
    _, text = editor(pid)
    assert text.startswith('qjv'), 'PageDown (the former key) moved the caret'
    print('ok: PageDown (the former key) does not move the caret')
    app.tap('Ctrl', 'Home')
    app.tap('F9')
    type_text('qjw')
    _, text = editor(pid)
    assert text.index('qjw') > text.index('\n'), 'F9 did not move the caret down a page'
    app.tap('F8')
    type_text('qju')
    _, text = editor(pid)
    assert text.index('qju') < text.index('qjw'), 'F8 did not move the caret up'
    print('ok: F9 / F8 move the caret a page down / up')

    app.tap('F1')
    names, _ = app.wait_for(pid, lambda names, _: {'F7', 'F8 / F9', 'F6'} <= names,
                            'F1 shows the new keys on the editor page')
    assert not {'Tab', 'PageUp / PageDown'} & names, names
    print('ok: help shows F7, F8 / F9, F6')
    app.tap('Escape')
    app.wait_for(pid, lambda names, _: 'F7' not in names, 'Escape closes the help')

    app.tap('Ctrl', 'Shift', 'P')
    search = app.catalog_texts('m024')
    app.wait_for(pid, lambda names, _: bool(search & names), 'the Snippets window opens')
    type_text(SURFACE)
    app.wait_for(pid, lambda _, values: SURFACE in values, 'the filter receives the name')
    app.tap('Enter')
    deadline = time.monotonic() + 10
    while not (hwnd := surface_window(pid)):
        assert time.monotonic() < deadline, 'the repl.ui surface window did not open'
        time.sleep(0.3)
    time.sleep(1.0)
    for old in [True, False]:
        app.user32.SetForegroundWindow(hwnd)
        time.sleep(0.5)
        assert app.user32.GetForegroundWindow() == hwnd, 'surface window is not in the foreground'
        if old:
            app.tap('Escape')
            time.sleep(1.0)
            assert surface_window(pid), 'Escape (the former key) closed the window'
            print('ok: Escape (the former key) leaves the repl.ui window open')
    app.tap('F6')
    deadline = time.monotonic() + 10
    while surface_window(pid):
        assert time.monotonic() < deadline, 'F6 did not close the repl.ui window'
        time.sleep(0.3)
    print('ok: F6 closes the repl.ui window')


def surface_window(pid):
    saved = app.TITLE
    app.TITLE = SURFACE
    try:
        return app.find_window(pid)
    finally:
        app.TITLE = saved


def with_test_keys(text):
    # Whole lines, since snippet_page_up = "PageUp" holds page_up = "PageUp".
    lines = text.splitlines(keepends=True)
    for before, after in [('focus_editor = "Tab"', 'focus_editor = "F7"'),
                          ('page_up = "PageUp"', 'page_up = "F8"'),
                          ('page_down = "PageDown"', 'page_down = "F9"'),
                          ('close_window = "Escape"', 'close_window = "F6"')]:
        found = [i for i, line in enumerate(lines) if line.rstrip('\r\n') == before]
        assert len(found) == 1, f'keybinds.toml has no line {before}'
        lines[found[0]] = lines[found[0]].replace(before, after)
    return ''.join(lines)


def main():
    original = app.KEYBINDS.read_bytes()
    with tempfile.TemporaryDirectory() as root:
        snippets = Path(root) / 'repl-plugin-player' / 'snippets'
        snippets.mkdir(parents=True)
        (snippets / f'{SURFACE}.js').write_text(SURFACE_CODE + '\n', encoding='utf-8')
        process = None
        try:
            app.KEYBINDS.write_bytes(with_test_keys(original.decode('utf-8')).encode('utf-8'))
            process = subprocess.Popen([str(app.EXE)], env=dict(os.environ, LOCALAPPDATA=root))
            hwnd = None
            for _ in range(100):
                time.sleep(0.2)
                hwnd = app.find_window(process.pid)
                if hwnd:
                    break
            assert hwnd, 'window not found'
            # The exe read keybinds.toml at startup; restore it before the slow part.
            app.KEYBINDS.write_bytes(original)
            time.sleep(2.0)
            app.user32.SetForegroundWindow(hwnd)
            time.sleep(0.5)
            assert app.user32.GetForegroundWindow() == hwnd, 'window is not in the foreground'
            check(process.pid)
        finally:
            app.KEYBINDS.write_bytes(original)
            if process:
                subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)],
                               capture_output=True)
                process.wait()
            time.sleep(0.5)  # Let the temporary directory's files be released.
    print('PASS focus_editor, page_up / page_down and close_window follow keybinds.toml')


if __name__ == '__main__':
    main()
