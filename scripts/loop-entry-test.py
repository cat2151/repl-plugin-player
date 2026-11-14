"""Real regression: ordinary playback -> standard loop clips -> immediate live updates.

Run after cargo build --release. Exercises a one-part and a two-part loop in fresh sessions.
The normal user data/log paths are used, as in scripts/e2e.py.
Entry scripts are test-owned so built-in snippets may change freely.
"""
from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[1]
LIBS = """var {loopPhraseBeats, prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');
var entryTrack = 0;
"""
PLAY = LIBS + """var midi = include('builtin/lib/midi.js');
midi.addFile(entryTrack, midi.writeMml(cmrt_chord.resolve_chord_or_mml('cde').mml));
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();"""
CLIPS = "uapmd.timeline.getClips(include('builtin/lib/tracks.js').idOf(0))"
START = """publishLoopParts(entryNext, 120);
uapmd.audio.setEngineEnabled(true);
if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();"""
ENTRIES = {
    'one part': LIBS + """var entryNext = Object.assign({}, globalThis.loopParts || {});
entryNext.main = prepareLoopPart(entryTrack, 'cegb');
""" + START,
    'two parts': LIBS + """if (!uapmd.sequencer.getTrackInfos().some(t => t.nodes.some(n => n.instanceId === globalThis.entrySecondId)))
    globalThis.entrySecondId = uapmd.instancing.create('CLAP', 'org.surge-synth-team.surge-xt');
if (entrySecondId < 0) throw new Error('second instrument creation failed');
var entryNext = {main: prepareLoopPart(entryTrack, 'Isus4 I'),
    second: prepareLoopPart(include('builtin/lib/plugins.js').trackFor(entrySecondId), '>c1c1')};
""" + START,
}


def check(session_type, name):
    entry = ENTRIES[name]
    session = session_type()
    try:
        session.request('eval', code=PLAY)
        before = session.evaluate(CLIPS)
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
        # The loop range is UAPMD transport state.
        session.request('wait', code='uapmd.timeline.getState().loopEnabled && uapmd.timeline.getState().isPlaying')
        assert session.evaluate('[entryStops,entryStarts]') == [0, 0]
        assert len(session.evaluate(CLIPS)) == len(before) + 1
        # Reloading the helper and the snippet must keep the running loop intact.
        session.request('eval', code=entry)
        assert session.evaluate('[entryStops,entryStarts]') == [0, 0]
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
        assert session.evaluate('[entryStops,entryStarts]') == [0, 0]
        saved = session.evaluate('JSON.stringify(loopParts)')
        clips = session.evaluate(CLIPS)
        error = session.request('eval', ok=False, code="""publishLoopParts({
            main:{trackIndex:entryTrack,filepath:'X:/missing-loop.mid',beats:4}}, 120);""")
        assert 'file' in error.lower(), error
        assert session.evaluate('JSON.stringify(loopParts)') == saved
        assert session.evaluate('[entryStops,entryStarts]') == [0, 0]
        assert session.evaluate('uapmd.timeline.getState().isPlaying') is True
        assert session.evaluate(CLIPS) == clips
        session.finish(failure=True)
        print(f'PASS {name}: entry/reload/update keeps playing; invalid import preserves clips', flush=True)
    finally:
        session.kill()


if __name__ == '__main__':
    session_type = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    for name in ENTRIES:
        check(session_type, name)
