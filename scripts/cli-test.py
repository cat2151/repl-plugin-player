"""Real application CLI regression. Generated scripts/data stay outside the repo.

Run after python scripts/build.py. Uses normal Ready (silent engine preparation), then no audio
playback. Briefly opens plugin editors through test-owned scripts, then runs every built-in snippet
in its own session and checks only that each runs without error.
Default includes normal-startup preset application and repeated-load acceptance.
--core checks the independent CLI features only and does not claim that preset
gate passes.
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
import xml.etree.ElementTree as XML

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target/release/repl-plugin-player.exe'
PRESETS = runpy.run_path(str(ROOT / 'patches/uapmd/clap-presets/regression.py'))
WINDOWS = runpy.run_path(str(ROOT / 'scripts/cli-windows-test.py'))
# Test-owned topology: a clip on the Surge track, an instrument on a new track, an effect after Surge.
JOBS = {'cliInstrumentId': ('cli-test-instrument', 'CLAP', 'org.baconpaul.six-sines', -1),
        'cliEffectId': ('cli-test-effect', 'CLAP', 'michaelwillis.dragonfly.hall', 'cliTrack')}
INSTRUMENT = 'include("builtin/lib/plugins.js").instrumentOn(0)'
SETUP = """globalThis.cliTrack = 0;
if (uapmd.timeline.createEmptyMidiClip(cliTrack, 0, 480, 120) < 0) throw new Error('clip creation failed');
""" + ''.join(f"uapmd.instancing.startCreateJob('{key}', '{format_}', '{plugin}', {track});\n"
              for key, format_, plugin, track in JOBS.values())
CREATED = ' && '.join(f"""(() => {{var j = uapmd.instancing.getCreateJob('{job[0]}');
    if (j.state === 'failed') throw new Error(j.error); return j.state === 'completed';}})()""" for job in JOBS.values())
# Surge and the effect editors are the two windows cli-windows-test.py expects.
SHOW = ''.join(f"globalThis.{name} = uapmd.instancing.startCreateJob('{key}', '{format_}', '{plugin}', {track}).instanceId;\n"
               for name, (key, format_, plugin, track) in JOBS.items()) + \
    'uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).showUI(); uapmd.instance(cliEffectId).showUI();'
# Test-owned preset choice; the preset snippet may select anything.
PRESET = 'Attacky'
LOAD = ('globalThis.cliPresetJob = uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).loadPreset('
        f'cliPresets.find(p => p.name === {json.dumps(PRESET)}).index)')
LOADED = ("(() => {var j = uapmd.mutations.getJob(cliPresetJob.jobId);"
          "if (j.state === 'failed') throw new Error(j.error); return j.state === 'completed';})()")


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
    # try-patch-browse opens the real player's browser and waits for a person to close it;
    # E2E patch-browse covers that flow with a stand-in player.
    names = sorted(path.stem for path in (ROOT / 'snippets/builtin').glob('*.js')
                   if path.stem != 'try-patch-browse')
    assert names
    session = Session(seconds=600)
    failed = False

    def run(name):
        nonlocal failed
        reply = session.request('snippet', group='Built-in', name=name, ok=None)
        failed |= not reply['ok']
        return reply['ok'] or 'normal GUI' in reply['error'], reply

    try:
        rerun = [name for name in names if not run(name)[0]]
        for name in rerun:
            end = time.monotonic() + 30
            while True:
                done, reply = run(name)
                if done:
                    break
                assert time.monotonic() < end, (name, reply)
                time.sleep(.5)
        events = session.finish(failure=failed)
        assert sum(e['event'] == 'run_input' and e['details']['scope'] == 'snippet' for e in events) >= len(names)
    finally:
        session.kill()
    print(f'PASS CLI every built-in snippet ({len(names)}) runs without error; repl.ui reports GUI-only')


def check():
    if sys.argv[1:] not in ([], ['--core']):
        raise SystemExit('Usage: python scripts/cli-test.py [--core]')
    core_only = sys.argv[1:] == ['--core']
    with tempfile.TemporaryDirectory(prefix='repl-cli-') as temporary:
        directory = Path(temporary)
        one_shot(directory)
        session = Session()
        try:
            script = directory / 'generated-check.js'
            script.write_text("""globalThis.cliMarker = '日本語';
if (typeof cmrt_chord.resolve_chord_or_mml !== 'function') throw new Error('Rust API missing');
globalThis.cliPresets = uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).getPresets();
for (var name of ['µcomputer', 'Sunrise Breakfast at the Digital Café', 'Wavetextures™ Pro Kit'])
    // Provider metadata and its supported filename fallback may differ in case.
    if (!cliPresets.some(p => p.name.toLowerCase() === name.toLowerCase())) throw new Error('Missing Unicode preset: ' + name);
({marker: cliMarker, presets: cliPresets.length});""", encoding='utf-8')
            result = session.request('script', path=str(script))['value']
            assert result['marker'] == '日本語' and result['presets'] > 600, result
            session.evaluate(SETUP)
            session.request('wait', code=CREATED)
            session.evaluate(SHOW)
            session.evaluate('globalThis.cliTopology = JSON.stringify(uapmd.sequencer.getTrackInfos());'
                             'globalThis.cliClips = JSON.stringify(uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackIndex)));')
            retained = {}
            for label, instance in [('instrument', 'cliInstrumentId'), ('effect', 'cliEffectId')]:
                retained[label] = bytes(session.request('state', instanceId=session.evaluate(instance)))
            session.evaluate('globalThis.cliWaitCount = 0')
            session.request('wait', code='++cliWaitCount > 3')
            assert session.evaluate('cliWaitCount') == 4
            if core_only:
                assert PRESETS['patch'](bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT)))))[0] == 'Init Saw'
            if not core_only:
                before = PRESETS['patch'](bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT)))))
                session.evaluate(LOAD)
                session.request('wait', code=LOADED)
                factory = PRESETS['patch'](Path(f'C:/ProgramData/Surge XT/patches_factory/Basses/{PRESET}.fxp').read_bytes()[60:])

                def check_state(prior, expected_factory=factory, display_name=None):
                    state = bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT))))
                    actual = PRESETS['patch'](state)
                    expected = PRESETS['expected_parameters'](expected_factory, prior)
                    expected_name = expected_factory[0] if display_name is None else display_name
                    assert actual[0] == expected_name, (actual[0], expected_name)
                    assert expected.keys() <= actual[1].keys(), 'Missing original parameters'
                    mismatches = [(k, v, actual[1][k]) for k, v in expected.items()
                                  if abs(actual[1][k] - v) > 1e-6 * max(1, abs(v))]
                    assert not mismatches, mismatches[:5]
                    if expected_factory[2] <= 22:
                        length = int.from_bytes(state[4:8], 'little')
                        parameters = XML.fromstring(state[32:32 + length].rstrip(b'\0')).find('parameters')
                        for key, value in expected_factory[1].items():
                            if key.startswith('fx') and key.endswith('_type') and value == 20:
                                assert parameters.find(key[:-5] + '_p11').get('deactivated') == '1'
                    return actual

                loaded = check_state(before)
                # Force an edit that changes an original FXP parameter, then reload
                # the same preset/name. Job completion must follow actual application.
                candidates = session.evaluate('uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).getParameters()')
                cutoff = next(p for p in candidates if 'cutoff' in p['name'].lower())
                parameter = cutoff['id']
                session.evaluate(f'uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).setParameterValue({parameter}, 0.37)')
                changed = PRESETS['patch'](bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT)))))
                assert changed[1] != loaded[1], 'Reload check did not change a stored parameter'
                for _ in range(2):
                    session.evaluate(LOAD)
                    session.request('wait', code=LOADED)
                    loaded = check_state(changed)
                for name in ('µcomputer', 'Sunrise Breakfast at the Digital Café', 'Wavetextures™ Pro Kit'):
                    selected = session.evaluate(f'cliPresets.find(p => p.name === {json.dumps(name)})')
                    files = [p for folder in ('patches_factory', 'patches_3rdparty')
                             for p in (Path('C:/ProgramData/Surge XT') / folder).rglob('*.fxp')
                             if p.stem.casefold() == name.casefold()]
                    assert len(files) == 1, (name, files)
                    reference = PRESETS['patch'](files[0].read_bytes()[60:], factory=True)
                    prior = PRESETS['patch'](bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT)))))
                    job = session.evaluate(f'globalThis.cliUnicodeJob = uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).loadPreset({selected["index"]})')
                    session.request('wait', code="(() => {var j = uapmd.mutations.getJob(cliUnicodeJob.jobId); if (j.state === 'failed') throw new Error(j.error); return j.state === 'completed';})()")
                    assert job['jobId'] > 0
                    # Surge 1.3.4 loadPatch passes the filesystem-derived patch
                    # list name to loadPatchByPath; it can differ in case from
                    # the FXP XML metadata (e.g. Sunrise ... At The ...).
                    check_state(prior, reference, files[0].stem)
                for invalid in (-1, 2147483647):
                    job = session.evaluate(f'uapmd.instance(include("builtin/lib/plugins.js").instrumentOn(0)).loadPreset({invalid})')
                    assert job['state'] == 'failed' and job['error'], job
                    assert session.evaluate('6 * 7') == 42
                # A valid load after rejected jobs proves mutation/runtime recovery.
                prior = PRESETS['patch'](bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT)))))
                session.evaluate(LOAD)
                session.request('wait', code=LOADED)
                check_state(prior)
            assert session.evaluate('JSON.stringify(uapmd.sequencer.getTrackInfos()) === cliTopology')
            assert session.evaluate('JSON.stringify(uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackIndex))) === cliClips')
            assert session.request('eval', code='void 0') == {'undefined': True, 'value': None}
            assert session.evaluate('uapmd.audio.isEngineEnabled()') is False
            for label, instance in [('instrument', 'cliInstrumentId'), ('effect', 'cliEffectId')]:
                assert bytes(session.request('state', instanceId=session.evaluate(instance))) == retained[label], label
            WINDOWS['check'](session)
            events = session.finish()
            ready = next(i for i, e in enumerate(events) if e['event'] == 'ready')
            assert not any(e['event'] == 'api' and e['details']['call']['api'] == 'uapmd.audio.setEngineEnabled'
                           and e['details']['call'].get('args') == [True] for e in events[ready + 1:])
            print(f"PASS CLI normal Ready, generated UTF-8 script, {result['presets']} Unicode preset entries, retained globals/topology, UI coexistence, cleanup")
            if core_only:
                print('NOT CHECKED by --core: stopped-Surge preset application/reload acceptance; run without --core for its explicit result')
            else:
                print('PASS preset acceptance: actual factory values, edited same-preset reloads, three Unicode loads, failed jobs/recovery, byte-equal other instrument/effect states; no engine/device playback')
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
                if command == 'state':
                    assert PRESETS['patch'](bytes(session.request('state', instanceId=int(session.evaluate(INSTRUMENT)))))[0] == 'Init Saw'
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
