"""Real application CLI regression. Generated scripts/data stay outside the repo.

Run after python scripts/build.py. Uses normal Ready (silent engine preparation), then no audio
playback. Briefly opens plugin editors through test-owned scripts, then runs every built-in snippet
in its own session and checks only that each runs without error.
"""
import json
from pathlib import Path
import queue
import runpy
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target/release/repl-plugin-player.exe'
WINDOWS = runpy.run_path(str(ROOT / 'scripts/cli-windows-test.py'))
# Test-owned topology: a clip on the Surge track, an instrument on a new track, an effect after Surge.
JOBS = {'cliInstrumentId': ('cli-test-instrument', 'CLAP', 'org.baconpaul.six-sines', "''"),
        'cliEffectId': ('cli-test-effect', 'CLAP', 'michaelwillis.dragonfly.hall',
                        "include('builtin/lib/tracks.js').idOf(cliTrack)")}
SETUP = """globalThis.cliTrack = 0;
if (!uapmd.timeline.createEmptyMidiClip(include('builtin/lib/tracks.js').idOf(cliTrack), 0, 480, 120))
    throw new Error('clip creation failed');
""" + ''.join(f"uapmd.instancing.startCreateJob('{key}', '{format_}', '{plugin}', {track});\n"
              for key, format_, plugin, track in JOBS.values())
CREATED = ' && '.join(f"""(() => {{var j = uapmd.instancing.getCreateJob('{job[0]}');
    if (j.state === 'failed') throw new Error(j.error); return j.state === 'completed';}})()""" for job in JOBS.values())
# Surge and the effect editors are the two windows cli-windows-test.py expects.
SHOW = ''.join(f"globalThis.{name} = uapmd.instancing.startCreateJob('{key}', '{format_}', '{plugin}', {track}).instanceId;\n"
               for name, (key, format_, plugin, track) in JOBS.items()) + \
    'uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).showUI(); uapmd.instance(cliEffectId).showUI();'


def launch(*args, **kwargs):
    return subprocess.Popen([str(EXE), '--lang', 'en', *args], cwd=ROOT, text=True, encoding='utf-8',
                            creationflags=subprocess.CREATE_NO_WINDOW, **kwargs)


class Session:
    def __init__(self, seconds=180):
        self.child = launch('--automation', '--timeout', str(seconds),
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.lines = queue.Queue()
        self.counter = 0

        def read():
            try:
                for line in self.child.stdout:
                    self.lines.put(json.loads(line))
            except Exception as error:
                self.lines.put(error)
            finally:
                self.lines.put(None)

        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()
        try:
            ready = self.receive(130)
            assert ready['event'] == 'ready' and ready['ok'], ready
            self.log = Path(ready['log'])
        except Exception:
            self.kill()
            raise

    def receive(self, timeout=35):
        result = self.lines.get(timeout=timeout)
        assert result is not None and not isinstance(result, Exception), result
        return result

    def request(self, command, ok=True, **args):
        self.counter += 1
        self.child.stdin.write(json.dumps(dict(command=command, id=self.counter, **args)) + '\n')
        self.child.stdin.flush()
        reply = self.receive()
        assert reply['id'] == self.counter, reply
        if ok is None:
            return reply
        assert reply['ok'] is ok, reply
        return reply.get('result') if ok else reply['error']

    def evaluate(self, code):
        return self.request('eval', code=code)['value']

    def finish(self, failure=False, eof=False):
        if eof:
            self.child.stdin.close()
        else:
            self.request('close')
        closed = self.receive()
        assert closed['event'] == 'closed' and closed['ok'], closed
        self.child.wait(timeout=20)
        assert (self.child.returncode != 0) is failure, self.child.returncode
        events = [json.loads(line) for line in self.log.read_text(encoding='utf-8').splitlines()]
        assert any(e['event'] == 'ready' for e in events), 'Normal Ready path was bypassed'
        assert any(e['event'] == 'init_evaluated' and 'error' not in e['details']['result'] for e in events)
        assert any(e['event'] == 'cleanup' and e['details']['result']['ok'] for e in events)
        return events

    def kill(self):
        if self.child.poll() is None:
            self.child.kill()
            self.child.wait(timeout=10)
        self.child.stdin.close()
        self.child.stdout.close()
        self.child.stderr.close()


def one_shot(directory):
    def execute(path):
        with launch('--script', str(path), stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
            try:
                stdout, stderr = child.communicate(timeout=150)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
                raise
            return child.returncode, [json.loads(line) for line in stdout.splitlines()], stderr

    script = directory / '日本語 script.js'
    script.write_text("\ufeffglobalThis.cliText = '日本語'; ({text: cliText, api: typeof cmrt_chord.resolve_chord_or_mml})", encoding='utf-8')
    code, events, stderr = execute(script)
    assert code == 0, (code, events, stderr)
    assert [e['event'] for e in events] == ['ready', 'result', 'closed'], events
    assert events[1]['result'] == {'value': {'text': '日本語', 'api': 'function'}, 'undefined': False}
    assert events[-1]['ok']
    script.write_text("throw new Error('one-shot-failure')", encoding='utf-8')
    code, events, _ = execute(script)
    assert code != 0 and any(e['event'] == 'closed' and e['ok'] for e in events), events
    assert any('one-shot-failure' in (e.get('error') or '') for e in events), events


def builtins():
    """Run every built-in snippet through the CLI snippet command; only errors are checked.

    Snippets may depend on globals of earlier ones and on async creation, and one snippet's
    state may conflict with another's in a fixed order, so each must succeed at least once:
    in the first pass, or when rerun (retrying while jobs settle) after all others have run.
    repl.ui is GUI-only, so its error is the expected CLI outcome.
    """
    # These open the real player's browser and wait for a person to close it;
    # E2E patch-browse and browser-change cover those flows with a stand-in player.
    waits_for_person = {'try-patch-browse', 'try-change-patch', 'try-change-effect',
                        'easy-change-patch', 'easy-change-effect'}
    needs_microphone = {'start-recording', 'stop-recording'}
    gui_only = {'show-recording-monitor'}
    names = sorted(path.stem for path in (ROOT / 'snippets/builtin').glob('*.js')
                   if path.stem not in waits_for_person | needs_microphone | gui_only)
    assert names
    session = Session(seconds=600)
    failed = False

    def run(name):
        nonlocal failed
        reply = session.request('snippet', group='Built-in', name=name, ok=None)
        failed |= not reply['ok']
        return reply['ok'] or 'normal GUI' in reply['error'], reply

    try:
        # Rounds over all remaining ones: one may need the state a later one leaves.
        rerun = [name for name in names if not run(name)[0]]
        end = time.monotonic() + 60
        while rerun:
            replies = {name: run(name) for name in rerun}
            rerun = [name for name, (done, _) in replies.items() if not done]
            assert not rerun or time.monotonic() < end, {name: replies[name][1] for name in rerun}
            time.sleep(.5)
        events = session.finish(failure=failed)
        assert sum(e['event'] == 'run_input' and e['details']['scope'] == 'snippet' for e in events) >= len(names)
    finally:
        session.kill()
    print(f'PASS CLI every built-in snippet ({len(names)}) runs without error; repl.ui reports GUI-only')


def check():
    if sys.argv[1:]:
        raise SystemExit('Usage: python scripts/cli-test.py')
    with tempfile.TemporaryDirectory(prefix='repl-cli-') as temporary:
        directory = Path(temporary)
        one_shot(directory)
        session = Session()
        try:
            script = directory / 'generated-check.js'
            script.write_text("""globalThis.cliMarker = '日本語';
if (typeof cmrt_chord.resolve_chord_or_mml !== 'function') throw new Error('Rust API missing');
({marker: cliMarker});""", encoding='utf-8')
            result = session.request('script', path=str(script))['value']
            assert result['marker'] == '日本語', result
            session.evaluate(SETUP)
            session.request('wait', code=CREATED)
            session.evaluate(SHOW)
            session.evaluate('globalThis.cliTopology = JSON.stringify(uapmd.sequencer.getTrackInfos());'
                             'globalThis.cliClips = JSON.stringify(uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackId)));')
            retained = {}
            for label, instance in [('instrument', 'cliInstrumentId'), ('effect', 'cliEffectId')]:
                retained[label] = bytes(session.request('state', instanceId=session.evaluate(instance)))
            session.evaluate('globalThis.cliWaitCount = 0')
            session.request('wait', code='++cliWaitCount > 3')
            assert session.evaluate('cliWaitCount') == 4
            assert session.evaluate('JSON.stringify(uapmd.sequencer.getTrackInfos()) === cliTopology')
            assert session.evaluate('JSON.stringify(uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackId))) === cliClips')
            assert session.request('eval', code='void 0') == {'undefined': True, 'value': None}
            assert session.evaluate('uapmd.audio.isEngineEnabled()') is False
            for label, instance in [('instrument', 'cliInstrumentId'), ('effect', 'cliEffectId')]:
                assert bytes(session.request('state', instanceId=session.evaluate(instance))) == retained[label], label
            WINDOWS['check'](session)
            events = session.finish()
            ready = next(i for i, e in enumerate(events) if e['event'] == 'ready')
            assert not any(e['event'] == 'api' and e['details']['call']['api'] == 'uapmd.audio.setEngineEnabled'
                           and e['details']['call'].get('args') == [True] for e in events[ready + 1:])
            print('PASS CLI normal Ready, generated UTF-8 script, retained globals/topology, UI coexistence, cleanup')
        finally:
            session.kill()
        builtins()
        session = Session()
        try:
            for command, args, expected in [
                ('eval', {'code': 'const = ;'}, ''),
                ('snippet', {'group': 'Built-in', 'name': 'missing-cli-test'}, 'Snippet not found'),
                ('script', {'path': str(directory / 'missing.js')}, 'missing.js'),
                ('state', {'instanceId': 2147483647}, 'missing'),
                ('state', {'instanceId': -1}, 'nonnegative'),
                ('state', {}, 'nonnegative'),
                ('invalid', {}, 'Unknown automation command')]:
                assert expected in session.request(command, ok=False, **args)
                assert session.evaluate('6 * 7') == 42
            session.finish(failure=True, eof=True)
        finally:
            session.kill()
        session = Session(seconds=8)
        try:
            error = session.request('wait', ok=False, code='false')
            assert 'timed out' in error, error
            closed = session.receive()
            assert closed['event'] == 'closed' and closed['ok'], closed
            session.child.wait(timeout=15)
            assert session.child.returncode != 0
        finally:
            session.kill()
        print('PASS CLI errors recover in one process, nonzero failure, EOF cleanup, timeout cleanup')
        session = Session()
        try:
            WINDOWS['quit_thread'](session)
        finally:
            session.kill()
        print('PASS CLI stdin-idle WM_NULL response, posted WM_CLOSE/reopen, WM_QUIT cleanup; normal GUI retains winit')


if __name__ == '__main__':
    check()
