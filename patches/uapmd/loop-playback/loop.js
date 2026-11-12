// The loop-playback patch on real plugin input: a fixed four-beat loop entered
// while ordinary transport runs, a rounded fractional period with trailing
// rests, and the tutorial reset. `loop-update.js` adds updates while it plays.
// Test-owned phrases: editable snippet contents are never expectations;
// pitches, onsets and periods come from the MML converter.

// %% helpers, native loop checks and a fresh plugin-input trace
globalThis.loopCase = (() => {
    const loopTrack = 0;
    const main = 'cegb';
    const loop = include('builtin/lib/loop.js');

    function play() {
        uapmd.audio.setEngineEnabled(true);
        if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
    }

    // The test-owned entry: `mml` as part `main` at 120 BPM, then playing.
    function publish(mml) {
        const next = Object.assign({}, globalThis.loopParts || {});
        next.main = loop.prepareLoopPart(loopTrack, mml);
        loop.publishLoopParts(next, 120);
        play();
    }

    // Each track with its nodes and clips, as the timeline view reads them.
    const tracks = () => uapmd.sequencer.getTrackInfos().map(track => ({
        trackIndex: track.trackIndex, nodes: track.nodes, clips: e2e.clips(track.trackIndex)
    }));

    return {loopTrack, main, loop, play, publish, tracks, sampleRate: uapmd.sequencer.getSampleRate()};
})();
repl.test.loopChecks();
loopCase.before = loopCase.tracks();
repl.test.loopTrace(true);
loopCase.sampleRate;

// %% ordinary transport already running
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% first loop publication
loopCase.publish(loopCase.main);

// %% fixed four-beat loop
loopCase.fixed = (() => {
    const {peak} = e2e.observe(6500);
    e2e.assert(peak > 0.001, 'Loop produced no output signal');
    const notes = e2e.ons(e2e.trace().events);
    e2e.assert(notes.length >= 12, `Too few loop notes: ${JSON.stringify(notes)}`);
    const origin = notes[0][0];
    const main = e2e.phrase(loopCase.main);
    notes.forEach(([frame, note], i) => e2e.assert(
        note === main.pitch(i) && Math.abs(frame - origin - main.at(i, main.period)) <= 1,
        `Loop timing/sequence mismatch at ${i}: ${JSON.stringify(notes)}`));
    e2e.assert(e2e.same(loopCase.tracks(), loopCase.before), 'Loop changed existing clips or plugin chain');
    return {origin, count: notes.length, peak};
})();

// %% stop - the fixed loop
e2e.stopped();
repl.test.loopTrace(true);

// %% a rounded sample period with trailing rests, not a block multiple
{
    const next = Object.assign({}, globalThis.loopParts || {});
    next.main = loopCase.loop.prepareLoopPart(loopCase.loopTrack, 'l16 crrr');
    repl.timeline.publishLoopClips(Object.keys(next).map(k => next[k]), 1.234567, 120);
    globalThis.loopParts = next;
    loopCase.play();
}

// %% the fractional period repeats without drift
(() => {
    e2e.wait(3000);
    const single = e2e.phrase('l16 crrr').pitch(0);
    const period = Math.round(1.234567 * loopCase.sampleRate / 2);
    const notes = e2e.ons(e2e.trace().events);
    e2e.assert(notes.length >= 4 && notes.every(([, note]) => note === single)
        && notes.every(([frame], i) => i === 0 || frame - notes[i - 1][0] === period),
        `Fractional loop drift: period ${period}, ${JSON.stringify(notes)}`);
    return {period, count: notes.length};
})();

// %% stop - the fractional loop
e2e.stopped();

// %% loop before the tutorial reset
repl.test.loopTrace(true);
loopCase.publish(loopCase.main);
e2e.wait(1200);
e2e.assert(e2e.ons(e2e.trace().events).length > 0, 'Loop did not play before reset');

// %% tutorial reset
include('builtin/lib/tutorial.js').reset();

// %% stop - the input trace restarts only with audio stopped
e2e.stopped();
repl.test.loopTrace(true);

// %% a later phrase, longer than the old loop, at position 0
{
    for (const track of uapmd.sequencer.getTrackInfos())
        for (const node of track.nodes.filter(node => node.instanceId >= 0))
            uapmd.sequencer.setPluginBypassed(node.instanceId, false);
    const laterPath = getGeneratedMidiPath();
    mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('l2 cde'), laterPath);
    const clipTrack = 0;
    const clip = uapmd.timeline.addMidiClip(clipTrack, 0, laterPath);
    if (!clip.success) throw new Error(clip.error);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}

// %% the later phrase plays through without folding
(() => {
    const later = e2e.phrase('l2 cde');
    const span = later.at(later.pitches.length - 1, later.period) / loopCase.sampleRate;
    e2e.wait(span * 1000 + 800);
    const state = uapmd.timeline.getState();
    // Folding never reaches the later pitches; repeats of one pitch before the next are ignored.
    const notes = e2e.ons(e2e.trace().events, e2e.instrument())
        .filter(([, note], i, all) => i === 0 || note !== all[i - 1][1]);
    e2e.assert(state.loopEnabled === false && state.loopEndSamples === 0 && globalThis.loopParts === undefined,
        `Loop survived reset: ${JSON.stringify(state)}, loopParts cleared: ${globalThis.loopParts === undefined}`);
    const pitches = notes.map(([, note]) => note);
    e2e.assert(pitches[0] === later.pitches[0], `First note missing after reset: ${JSON.stringify(notes)}`);
    e2e.assert(e2e.same(pitches, later.pitches), `Phrase folded after reset: ${JSON.stringify(notes)}`);
    return notes;
})();

// %% stop - after the reset
e2e.stopped();
