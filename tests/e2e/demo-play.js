// `builtin/lib/tutorial.js` plays a phrase as one clip with no leading rest,
// the plugin receives every note and the first one sounds, play keeps the
// instrument's edited sound, and play leaves loop playback.
// Test-owned phrases and settings: editable snippet contents are never expectations.

// %% helpers
globalThis.demoPlay = (() => {
    const {assert, clips, notes} = e2e;

    const usePatch = patch => include('builtin/lib/patch.js').use(
        {format: 'CLAP', plugin: 'Surge XT', patch, track: 0}).instanceId;

    // Init, then play: as a demo cell does.
    function init() {
        globalThis.testDemo = include('builtin/lib/tutorial.js');
        testDemo.init({instanceId: usePatch('patches_factory/Templates/Init Saw.fxp'), bpm: 120});
    }

    // The pitches the phrase should play, from the MML parser itself.
    const pitches = source => mmlabc_to_smf.pass3_events.ast_to_events(
        mmlabc_to_smf.pass2_ast.tokens_to_ast(mmlabc_to_smf.pass1_parser.parse_mml(
            cmrt_chord.resolve_chord_or_mml(source).mml)), true)
        .filter(e => e.type === 'note_on').map(e => e.note);

    // Checks the play that the previous cell started.
    function check(source) {
        const expected = pitches(source);
        assert(expected.length, 'No reference notes');
        const read = clips(0);
        assert(read.length === 1, `Demo did not replace its clip: ${JSON.stringify(read)}`);
        const played = notes(read[0].midi.events);
        assert(played.length && played[0].start === 0, 'Demo inserted a leading rest');
        const ends = played.map(n => n.end).filter(end => end !== undefined);
        assert(ends.length, 'No note ends');
        const seconds = Math.max(...ends) / read[0].midi.tickResolution * 0.5 + 0.2;
        const deadline = Date.now() + seconds * 1000;
        const delivered = new Set();
        let firstPeak = 0;
        while (Date.now() < deadline) {
            const active = repl.test.notes();
            const signal = repl.test.observe();
            assert(signal.finite, 'Non-finite demo output');
            if (active.includes(expected[0])) firstPeak = Math.max(firstPeak, signal.peak);
            active.forEach(pitch => delivered.add(pitch));
            e2e.wait(5);
        }
        const want = [...new Set(expected)].sort((a, b) => a - b);
        const got = [...delivered].sort((a, b) => a - b);
        assert(e2e.same(want, got) && firstPeak > 0.001,
            `Missing demo notes/first-note signal: ${source}, expected ${JSON.stringify(expected)}, delivered ${JSON.stringify(got)}, first peak ${firstPeak}`);
        assert(!(uapmd.timeline.getState().loopEnabled || typeof globalThis.loopParts !== 'undefined'),
            'Demo retained loop playback');
        return {source, pitches: expected, firstPeak};
    }

    return {init, check, usePatch};
})();

// %% first run and reinitialization in one evaluation
demoPlay.init();
testDemo.play('cfg');

// %% check
demoPlay.check('cfg');

// %% initialize and play again
demoPlay.init();
testDemo.play('cfg');

// %% check
demoPlay.check('cfg');

// %% edits made after initializing (like another cell or the plugin UI) survive play
uapmd.audio.setEngineEnabled(false);
demoPlay.usePatch('patches_factory/Basses/Attacky.fxp');

// %% edit the volume
(() => {
    const instance = uapmd.instance(e2e.instrument());
    const volume = instance.getParameters().find(p => p.name === 'Global Volume');
    instance.setParameterValue(volume.id, 0.8);
    const controls = instance.getParameters().filter(p => !p.isReadonly);
    globalThis.testValues = () => controls.map(p => ({id: p.id, name: p.name,
        value: uapmd.instance(e2e.instrument()).getParameterValue(p.id)}));
    globalThis.testBefore = testValues();
})();

// %% play without initializing
testDemo.play('F-G');

// %% check
demoPlay.check('F-G');

// %% play keeps the sound and the volume
(() => {
    const after = testValues();
    const changed = after.map((p, i) => ({...p, before: testBefore[i].value}))
        .filter(p => p.value !== p.before);
    e2e.assert(!changed.length, `play changed instrument parameters: ${JSON.stringify(changed)}`);
})();

// %% initializing explicitly loads its chosen patch again
demoPlay.init();
testDemo.play('cfg');

// %% check
demoPlay.check('cfg');

// %% replacing an actively playing clip
testDemo.play('c1');

// %% play over it
testDemo.play('cfg');

// %% check
demoPlay.check('cfg');

// %% entering from an active loop
(() => {
    const loop = include('builtin/lib/loop.js');
    const loopTrack = 0;
    loop.publishLoopParts({test: loop.prepareLoopPart(loopTrack, 'c1')}, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
})();

// %% play from the loop
testDemo.play('cfg');

// %% check
demoPlay.check('cfg');
