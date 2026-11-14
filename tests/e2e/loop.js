// Standard UAPMD clips with a session-only transport loop.
// %% prepare the clip and trace
globalThis.loopTest = {track:0, mml:'l4 >cg<cg'};
loopTest.lib = include('builtin/lib/loop.js');
loopTest.lib.publishLoopParts({main:loopTest.lib.prepareLoopPart(loopTest.track,loopTest.mml)},120);
loopTest.clips = e2e.clips(loopTest.track);
e2e.assert(loopTest.clips.length === 1 && loopTest.clips[0].midi.events.length > 0,
    'Loop notes must be ordinary timeline clips');
repl.test.loopTrace(true);

// %% play several passes
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();
loopTest.peak = e2e.observe(6500).peak;
{
    const phrase = e2e.phrase(loopTest.mml);
    const notes = e2e.ons(e2e.trace().events, e2e.instrument());
    e2e.assert(notes.length >= 12 && loopTest.peak > 0.001, 'Loop did not repeat with signal');
    const origin = notes[0][0];
    notes.forEach(([frame, note], i) => e2e.assert(
        note === phrase.pitch(i) && Math.abs(frame - origin - phrase.at(i,phrase.period)) <= 1,
        `UAPMD loop sequence/timing ${i}: ${JSON.stringify(notes)}`));
    const events = e2e.trace().events.filter(e => e.instance === e2e.instrument());
    for (let pass = 1; pass < Math.floor(notes.length / phrase.pitches.length); ++pass) {
        const boundary = origin + pass * phrase.period;
        const lastPitch = phrase.pitches[phrase.pitches.length - 1];
        e2e.assert(events.some(e => !e.on && e.note === lastPitch && Math.abs(e.frame - boundary) <= 2),
            `Missing terminal Note Off at loop boundary ${pass}: ${JSON.stringify(events)}`);
    }
    e2e.assert(e2e.same(loopTest.clips,e2e.clips(loopTest.track)), 'Playback changed the clips');
}

// %% disable repeat while playing
repl.timeline.clearLoop();
e2e.assert(!uapmd.timeline.getState().loopEnabled && uapmd.timeline.getState().isPlaying,
    'Clearing the range stopped playback or retained looping');
e2e.assert(e2e.same(loopTest.clips,e2e.clips(loopTest.track)), 'Disabling repeat deleted the notes');

// %% stop before range test
e2e.stopped();
repl.test.loopTrace(true);

// %% an arbitrary nonzero range and fractional period
{
    const sr = uapmd.sequencer.getSampleRate();
    loopTest.start = Math.round(sr / 4);
    loopTest.end = loopTest.start + Math.round(sr * 0.6172835);
    repl.timeline.setLoop(loopTest.start,loopTest.end);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    e2e.wait(3200);
    const firstPitch = e2e.phrase(loopTest.mml).pitches[0];
    const secondPitch = e2e.phrase(loopTest.mml).pitches[1];
    const ons = e2e.ons(e2e.trace().events,e2e.instrument());
    // The first pass starts at timeline zero; subsequent passes start at the range start.
    e2e.assert(ons[0][1] === firstPitch, 'Nonzero range skipped the initial timeline');
    const repeated = ons.filter(([, note]) => note === secondPitch);
    e2e.assert(repeated.length >= 4 && repeated.every(([frame],i) => i === 0 ||
        Math.abs(frame-repeated[i-1][0] - (loopTest.end-loopTest.start)) <= 2),
        `Nonzero fractional loop drift: ${JSON.stringify(ons)}`);
    const state = uapmd.timeline.getState();
    e2e.assert(state.playheadSamples >= loopTest.start && state.playheadSamples < loopTest.end,
        `Playhead escaped range: ${JSON.stringify(state)}`);
}

// %% stop
e2e.stopped();

// %% a Note On at the endpoint must stay outside the loop
{
    const lib = loopTest.lib;
    lib.publishLoopParts({main:lib.prepareLoopPart(loopTest.track, 'l4 cd')}, 120, true);
    const period = Math.round(uapmd.sequencer.getSampleRate() / 2);
    repl.timeline.setLoop(0, period);
    repl.test.loopTrace(true);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    e2e.wait(2300);
    const events = e2e.trace().events.filter(e => e.instance === e2e.instrument());
    const ons = events.filter(e => e.on);
    const pitches = e2e.phrase('l4 cd').pitches;
    e2e.assert(ons.length >= 4 && ons.every(e => e.note === pitches[0]),
        `Endpoint Note On leaked into loop: ${JSON.stringify(events)}`);
    for (let i = 1; i < ons.length; ++i) {
        const off = events.findIndex(e => !e.on && e.note === pitches[0] && Math.abs(e.frame - ons[i].frame) <= 2);
        e2e.assert(off >= 0 && off < events.indexOf(ons[i]),
            `Release missing or ordered after next pass: ${JSON.stringify(events)}`);
    }
}

// %% stop after endpoint filtering
e2e.stopped();
