"""Real regression: ordinary playback -> first loop publication -> live updates.

Run after cargo build --release. Exercises a one-part and a two-part loop in fresh sessions.
The normal user data/log paths are used, as in scripts/e2e.py.
Entry scripts are test-owned so built-in snippets may change freely.
"""
from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[1]
LIBS = """var {loopPhraseBeats, prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');
var entryTrack = uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === 0);
"""
PLAY = LIBS + """var midi = include('builtin/lib/midi.js');
midi.addFile(entryTrack.trackIndex, midi.writeMml(cmrt_chord.resolve_chord_or_mml('cde').mml));
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();"""
START = """publishLoopParts(entryNext, 120);
uapmd.audio.setEngineEnabled(true);
if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();"""
ENTRIES = {
    'one part': LIBS + """var entryNext = Object.assign({}, globalThis.loopParts || {});
entryNext.main = prepareLoopPart(entryTrack.trackIndex, 'cegb');
""" + START,
    'two parts': LIBS + """if (!uapmd.sequencer.getTrackInfos().some(t => t.nodes.some(n => n.instanceId === globalThis.entrySecondId)))
    globalThis.entrySecondId = uapmd.instancing.create('CLAP', 'org.surge-synth-team.surge-xt', -1);
if (entrySecondId < 0) throw new Error('second instrument creation failed');
var entryNext = {main: prepareLoopPart(entryTrack.trackIndex, 'Isus4 I'),
    second: prepareLoopPart(include('builtin/lib/plugins.js').trackFor(entrySecondId).trackIndex, '>c1c1')};
""" + START,
}


def check(session_type, name):
    entry = ENTRIES[name]
    session = session_type()
    try:
        session.request('eval', code=PLAY)
        before = session.evaluate('uapmd.timeline.getClips(0)')
        assert session.evaluate('uapmd.timeline.getState().isPlaying') is True
        # Count explicit transport mutations without altering their receivers/results.
        session.evaluate("""globalThis.entryStops = 0; globalThis.entryStarts = 0;
            for (const [method, counter] of [['stopPlayback','entryStops'],['startPlayback','entryStarts']]) {
                const original = uapmd.sequencer[method];
                uapmd.sequencer[method] = function(...args) {
                    globalThis[counter] += 1; return Reflect.apply(original,this,args);
                };
            }""")
        session.request('eval', code=entry)
        # Native status.revision changes only on boundary replacement, not initial adoption.
        session.request('wait', code='uapmd.timeline.getState().loopEnabled && uapmd.timeline.getState().isPlaying')
        assert session.evaluate('[entryStops,entryStarts]') == [1, 1]
        assert session.evaluate('uapmd.timeline.getClips(0)') == before
        # Reloading the helper and the snippet must keep the running loop intact.
        session.request('eval', code=entry)
        assert session.evaluate('[entryStops,entryStarts]') == [1, 1]
        assert session.evaluate('uapmd.timeline.getState().isPlaying') is True
        # Observe an actual period change rather than diagnostic revision bookkeeping.
        period = session.evaluate('uapmd.timeline.getState().loopEndSamples')
        old_beats = session.evaluate('Math.max(...Object.values(loopParts).map(p=>p.beats))')
        # Lengthen the longest part by one beat through the lib.
        session.evaluate(f"""var longer = Object.assign({{}}, loopParts);
            longer.main = prepareLoopPart(loopParts.main.trackIndex, 'c' + 'r'.repeat({round(old_beats * 2) + 1}));
            publishLoopParts(longer, 120);""")
        new_beats = session.evaluate('Math.max(...Object.values(loopParts).map(p=>p.beats))')
        assert new_beats != old_beats
        expected_period = round(period * new_beats / old_beats)
        session.request('wait', code=f'uapmd.timeline.getState().loopEndSamples === {expected_period}')
        assert session.evaluate('[entryStops,entryStarts]') == [1, 1]
        saved = session.evaluate('JSON.stringify(loopParts)')
        error = session.request('eval', ok=False, code="""var invalid = Object.assign({},loopParts);
            invalid.main = prepareLoopPart(loopParts.main.trackIndex,'t90 c4'); publishLoopParts(invalid, 120);""")
        assert 'tempo' in error, error
        assert session.evaluate('JSON.stringify(loopParts)') == saved
        assert session.evaluate('[entryStops,entryStarts]') == [1, 1]
        assert session.evaluate('uapmd.timeline.getState().isPlaying') is True
        assert session.evaluate('uapmd.timeline.getClips(0)') == before
        session.finish(failure=True)
        print(f'PASS {name}: first entry stops once; reload/update keeps playing; invalid update preserves map and clips', flush=True)
    finally:
        session.kill()


if __name__ == '__main__':
    session_type = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    for name in ENTRIES:
        check(session_type, name)
