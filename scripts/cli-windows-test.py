"""Window responsiveness checks for the app's CLI-owned Windows message loop."""
import ctypes as c
from ctypes import wintypes as w
import time

user32 = c.WinDLL('user32', use_last_error=True)
user32.FindWindowExW.argtypes = [w.HWND, w.HWND, w.LPCWSTR, w.LPCWSTR]
user32.FindWindowExW.restype = w.HWND
user32.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
user32.GetWindowThreadProcessId.restype = w.DWORD
user32.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.IsWindow.argtypes = [w.HWND]
user32.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.PostThreadMessageW.argtypes = [w.DWORD, w.UINT, w.WPARAM, w.LPARAM]
user32.SendMessageTimeoutW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT, c.POINTER(c.c_size_t)]
user32.SendMessageTimeoutW.restype = c.c_ssize_t
SHOW_SURGE = 'uapmd.instance(surgeId).showUI()'


def windows(pid):
    result, previous = [], None
    while True:
        hwnd = user32.FindWindowExW(None, previous, 'RemidyContainerWindow', None)
        if not hwnd:
            return result
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, c.byref(owner))
        if owner.value == pid:
            result.append(hwnd)
        previous = hwnd


def wait_until(condition, description):
    end = time.monotonic() + 3
    while not condition():
        assert time.monotonic() < end, description
        time.sleep(.02)


def responsive(hwnd):
    result = c.c_size_t()
    assert user32.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 1500, c.byref(result)), 'Window did not answer WM_NULL'


def check(session):
    wait_until(lambda: len(windows(session.child.pid)) == 2, 'Missing CLI plugin containers')
    handles = windows(session.child.pid)
    surge = None
    for hwnd in handles:
        responsive(hwnd)
        title = c.create_unicode_buffer(512)
        user32.GetWindowTextW(hwnd, title, len(title))
        if title.value.startswith('Surge XT'):
            surge = hwnd
    assert surge is not None, 'Surge container not found'
    # No requests are sent while waiting: stdin-idle CLI must dispatch posted
    # WM_CLOSE, not only sent WM_NULL or paint/timer messages from native pump.
    assert user32.PostMessageW(surge, 0x10, 0, 0)
    wait_until(lambda: not user32.IsWindowVisible(surge), 'Idle CLI starved posted WM_CLOSE')
    assert user32.IsWindow(surge), 'Close destroyed the retained editor'
    session.evaluate(SHOW_SURGE)
    wait_until(lambda: bool(user32.IsWindowVisible(surge)), 'Surge did not reopen')
    responsive(surge)


def quit_thread(session):
    session.evaluate(SHOW_SURGE)
    hwnd = windows(session.child.pid)[0]
    owner = w.DWORD()
    thread = user32.GetWindowThreadProcessId(hwnd, c.byref(owner))
    assert owner.value == session.child.pid
    assert user32.PostThreadMessageW(thread, 0x12, 0, 0)
    closed = session.receive()
    assert closed['event'] == 'closed' and closed['ok'], closed
    error = session.receive()
    assert error['event'] == 'error' and 'WM_QUIT' in error['error'], error
    session.child.wait(timeout=15)
    assert session.child.returncode != 0
