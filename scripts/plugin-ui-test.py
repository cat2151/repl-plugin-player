"""Installed-Surge regression for the public script API and native editor lifecycle.

Run after python scripts/build.py. Briefly opens editors; never starts audio. Test data and native
diagnostics live in a temporary directory outside the repository.
"""
import ctypes as c
from ctypes import wintypes as w
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
user32 = c.WinDLL('user32', use_last_error=True)
user32.FindWindowExW.argtypes = [w.HWND, w.HWND, w.LPCWSTR, w.LPCWSTR]
user32.FindWindowExW.restype = w.HWND
user32.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.IsWindow.argtypes = [w.HWND]
user32.GetClientRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
user32.GetWindowRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
user32.PeekMessageW.argtypes = [c.POINTER(w.MSG), w.HWND, w.UINT, w.UINT, w.UINT]
user32.TranslateMessage.argtypes = [c.POINTER(w.MSG)]
user32.DispatchMessageW.argtypes = [c.POINTER(w.MSG)]
user32.DispatchMessageW.restype = c.c_ssize_t
user32.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
# Test-owned scripts; built-in snippets may change freely.
CREATE_SURGE = "globalThis.uiInstrumentId = uapmd.instancing.create('CLAP', 'org.surge-synth-team.surge-xt', 0)"
SHOW_SURGE = 'uapmd.instance(uiInstrumentId).showUI()'
EFFECT = ("globalThis.uiEffectId = uapmd.instancing.create('CLAP', 'michaelwillis.dragonfly.hall', "
          "uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === 0).trackIndex);"
          "if (uiEffectId < 0) throw new Error('effect creation failed')")
SHOW_EFFECT = 'uapmd.instance(uiEffectId).showUI()'


def windows():
    result, previous = [], None
    while True:
        hwnd = user32.FindWindowExW(None, previous, 'RemidyContainerWindow', None)
        if not hwnd:
            return result
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, c.byref(owner))
        if owner.value == os.getpid():
            result.append(hwnd)
        previous = hwnd


def main():
    # Preserve Python's console handles: the bridge redirects CRT stdout/stderr.
    report = os.dup(1)
    temporary = sys.argv[1]
    data = Path(temporary)
    cache = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/uapmd/plugin-list-cache.json'
    if cache.is_file():
        shutil.copy2(cache, data / cache.name)
    dll = c.CDLL(str(ROOT / 'build/out/repl_bridge.dll'))
    import native_text
    text_callback = native_text.register(dll)
    dll.repl_request.argtypes = [c.c_char_p]
    dll.repl_request.restype = c.c_char_p

    def call(command, **arguments):
        reply = json.loads(dll.repl_request(json.dumps(dict(command=command, **arguments)).encode()))
        if not reply['ok']:
            raise RuntimeError(reply.get('error'))
        return reply

    def evaluate(code):
        result = call('eval', code=code)['result']
        return None if result == 'undefined' else json.loads(result)

    def pump(seconds=.2):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            call('poll')
            # No winit in this harness: supply its normal message loop here.
            message = w.MSG()
            for _ in range(64):
                if not user32.PeekMessageW(c.byref(message), None, 0, 0, 1):
                    break
                user32.TranslateMessage(c.byref(message))
                user32.DispatchMessageW(c.byref(message))
            time.sleep(.01)

    try:
        call('open', data=str(data), diagnostics=str(data / 'native'))
        evaluate((ROOT / 'src/script-api.js').read_text(encoding='utf-8'))
        deadline = time.monotonic() + 120
        while not call('poll')['scanned']:
            assert time.monotonic() < deadline, 'Scan timed out'
            pump()
        instance_id = evaluate(CREATE_SURGE)
        assert instance_id >= 0, 'Creation failed'
        parameters = evaluate(f'uapmd.instance({instance_id}).getParameters()')
        parameter = next(p for p in parameters if not p['isReadonly'] and not p['isDiscrete'])
        safety = call('parameter-safety', instanceId=instance_id, parameterId=parameter['id'])
        assert safety['epoch'] > 0 and safety['stableId'].isdigit(), safety
        assert safety['readonly'] == bool(safety['flags'] & (1 << 3)), safety
        assert safety['discrete'] == bool(safety['flags'] & 1), safety
        assert safety['min'] == parameter['minValue'] and safety['max'] == parameter['maxValue'], safety
        for target, index in [(2147483647, parameter['id']), (instance_id, 2147483647), (instance_id, -1)]:
            try:
                call('parameter-safety', instanceId=target, parameterId=index)
            except RuntimeError:
                pass
            else:
                raise AssertionError(f'Invalid parameter binding accepted: {target}, {index}')
        snippet = SHOW_SURGE
        evaluate(snippet)
        pump(.5)
        assert len(windows()) == 1, windows()
        hwnd = windows()[0]
        assert user32.IsWindowVisible(hwnd), 'Editor did not stay visible'
        child = user32.FindWindowExW(hwnd, None, None, None)
        assert child and user32.IsWindowVisible(child), 'No visible embedded editor'
        client, editor = w.RECT(), w.RECT()
        assert user32.GetClientRect(hwnd, c.byref(client))
        assert user32.GetWindowRect(child, c.byref(editor))
        assert client.right == editor.right - editor.left > 0, (client.right, editor.right - editor.left)
        assert client.bottom == editor.bottom - editor.top > 0
        try:
            evaluate('uapmd.instance(2147483647).showUI()')
            raise AssertionError('Invalid instance was accepted')
        except RuntimeError as error:
            assert 'Plugin instance not found' in str(error)
        evaluate(snippet)
        assert windows() == [hwnd], 'Repeated show recreated the container'
        evaluate('uapmd.instance(uiInstrumentId).hideUI()')
        assert not user32.IsWindowVisible(hwnd)
        evaluate(snippet)
        assert windows() == [hwnd] and user32.IsWindowVisible(hwnd)
        user32.PostMessageW(hwnd, 0x10, 0, 0)  # WM_CLOSE follows the user close-button path.
        pump()
        assert not user32.IsWindowVisible(hwnd), 'Close did not hide the editor'
        evaluate(snippet)
        assert windows() == [hwnd] and user32.IsWindowVisible(hwnd)
        # An effect editor uses the same API.
        evaluate(EFFECT)
        effect_snippet = SHOW_EFFECT
        evaluate(effect_snippet)
        pump()
        assert len(windows()) == 2 and all(user32.IsWindowVisible(h) for h in windows())
        evaluate(effect_snippet)
        assert len(windows()) == 2, 'Repeated effect show recreated the container'
        # Share the real-plugin process/message loop with the preset regression.
        import runpy
        preset_checks = runpy.run_path(str(ROOT / 'patches/uapmd/clap-presets/regression.py'))
        preset_result = preset_checks['check'](call, evaluate, pump, data, instance_id)
        os.write(report, (preset_result + '\n').encode('utf-8'))
        evaluate('uapmd.instance(uiEffectId).dispose()')
        pump()
        assert windows() == [hwnd], 'Effect disposal affected the instrument window'
        assert evaluate('uapmd.audio.isEngineEnabled()') is False
        evaluate('uapmd.instance(uiInstrumentId).dispose()')
        pump()
        assert not windows(), 'Disposed instance retained its container'
        try:
            evaluate(snippet)
            raise AssertionError('Missing instance was accepted')
        except RuntimeError as error:
            assert 'Plugin instance not found' in str(error)
        # Exit with a visible editor as well as testing explicit disposal.
        evaluate(CREATE_SURGE)
        evaluate(snippet)
        assert len(windows()) == 1
    finally:
        call('close')
    assert not windows(), 'Session shutdown retained its container'
    os.write(report, b'PASS plugin UI: embedded size, repeated show, hide, close, reopen, dispose, shutdown; audio inactive\n')
    os.close(report)


if __name__ == '__main__':
    if len(sys.argv) == 1:
        # Native CRT diagnostic streams stay open until process exit.
        # The parent owns cleanup so even a failing child leaves no temp logs.
        with tempfile.TemporaryDirectory(prefix='repl-plugin-ui-') as temporary:
            outcome = subprocess.run([sys.executable, __file__, temporary], timeout=180)
        raise SystemExit(outcome.returncode)
    error_report = os.dup(2)
    try:
        main()
    except Exception:
        os.write(error_report, traceback.format_exc().encode('utf-8'))
        raise SystemExit(1)
    finally:
        os.close(error_report)
