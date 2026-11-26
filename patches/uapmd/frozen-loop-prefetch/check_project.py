"""Read-only real project regression: freeze every clip track and compare loop-head samples.

Uses the real device (audible) and the native scope callback to capture the selected
track after freeze. No project saves; isolated native state and logs outside the repo.
"""
import argparse
import array
import ctypes as c
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[3]


def run_child(args):
    report = os.dup(1)  # Native session redirects CRT output.
    sys.path.insert(0, str(ROOT / 'scripts'))
    import native_text
    dll = c.CDLL(str(ROOT / 'build/out/uapmd_host.dll'))
    text_callback = native_text.register(dll)
    dll.repl_request.argtypes = [c.c_char_p]
    dll.repl_request.restype = c.c_char_p

    def call(command, **kwargs):
        result = json.loads(dll.repl_request(json.dumps(dict(command=command, **kwargs)).encode()))
        if not result['ok']:
            raise RuntimeError(result)
        return result

    def evaluate(code):
        result = call('eval', code=code)['result']
        return None if result == 'undefined' else json.loads(result)

    blocks = []
    callback_type = c.CFUNCTYPE(None, c.c_void_p, c.c_void_p, c.c_void_p,
                               c.c_size_t, c.c_uint32, c.c_void_p, c.c_size_t)

    @callback_type
    def capture(context, left, right, frames, doubles, events, words):
        blocks.append((bool(doubles), c.string_at(left, frames * (8 if doubles else 4))))

    dll.repl_set_scope.argtypes = [callback_type, c.c_void_p]
    cells = {}
    for cell in Path(__file__).with_name('project.js').read_text(encoding='utf-8').split('// %% ')[1:]:
        title, code = cell.split('\n', 1)
        cells[title.strip()] = code
    directory = Path(args.data)
    cache = Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/uapmd/plugin-list-cache.json'
    shutil.copy2(cache, directory / cache.name)
    try:
        call('open', data=str(directory), diagnostics=str(directory / 'native'))
        evaluate((ROOT / 'src/script-api.js').read_text(encoding='utf-8'))
        evaluate('globalThis.prefetchProject = ' + json.dumps(str(args.project.resolve())))
        evaluate('globalThis.prefetchTrackIndex = ' + str(args.track))
        evaluate(cells['load and freeze'])
        deadline = time.monotonic() + 240
        while not evaluate(cells['ready']):
            if time.monotonic() > deadline:
                raise TimeoutError('Freeze timed out')
            call('poll')
            time.sleep(.01)
        info = evaluate(cells['prepare loop'])
        if info['frames'] < info['rate'] * 2:
            raise ValueError('Project needs a loop longer than the disk lookahead')
        dll.repl_set_scope(capture, None)
        evaluate(cells['start'])
        deadline = time.monotonic() + info['frames'] / info['rate'] * 4 + 1
        while time.monotonic() < deadline:
            call('poll')
            time.sleep(.005)
        evaluate(cells['stop'])
        samples = array.array('f')
        for doubles, raw in blocks:
            samples.extend(array.array('d' if doubles else 'f', raw))
        window = int(info['rate'] * .1)
        # Inspect three actual wraps. The initial start is a separate cold-cache
        # transition and is explicitly reported instead of serving as a reference.
        initial_peak = max(map(abs, samples[:window]), default=0)
        heads = [samples[p * info['frames']:p * info['frames'] + window] for p in range(1, 4)]
        if any(len(head) != window for head in heads):
            raise AssertionError('Missing callbacks')
        peaks = [max(map(abs, head)) for head in heads]
        if min(peaks) < .0001:
            raise AssertionError(f'Silent head: {peaks}')
        errors = [max(abs(a-b) for a, b in zip(heads[0], head)) for head in heads[1:]]
        if max(errors) > 1e-6:
            raise AssertionError(f'Loop head differs: peaks={peaks}, maximum errors={errors}')
        message = dict(info, headPeaks=peaks, maxSampleErrors=errors, track=args.track,
                       initialStartPeak=initial_peak)
        os.write(report, ('PASS: ' + json.dumps(message) + '\n').encode())
    finally:
        call('close')
    # Keep callbacks alive until after native teardown.
    assert text_callback and capture


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('project', type=Path)
    parser.add_argument('--track', type=int, required=True, help='Zero-based track index to capture')
    parser.add_argument('--child', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--data', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.child:
        error_report = os.dup(2)
        try:
            run_child(args)
        except Exception:
            os.write(error_report, traceback.format_exc().encode('utf-8'))
            raise SystemExit(1)
        finally:
            os.close(error_report)
        return
    original = hashlib.sha256(args.project.read_bytes()).digest()
    try:
        with tempfile.TemporaryDirectory(prefix='repl-loop-head-') as temporary:
            subprocess.run([sys.executable, '-B', __file__, str(args.project), '--track', str(args.track),
                            '--child', '--data', temporary], cwd=ROOT, check=True, timeout=360)
    finally:
        if hashlib.sha256(args.project.read_bytes()).digest() != original:
            raise RuntimeError('Project archive changed')


if __name__ == '__main__':
    main()
