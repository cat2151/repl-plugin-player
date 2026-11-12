// Entering loop playback after the tutorial `play()` clears the ordinary clips on
// every track, and from the loop's first note on only loop notes reach the plugin.
// Test-owned phrases: editable snippet contents are never expectations.

// %% native loop checks, the phrases, and a fresh plugin-input trace
repl.test.loopChecks();
globalThis.loopReplaces = {playMml: 'c', loopMml: 'gggg'};
loopReplaces.played = e2e.phrase(loopReplaces.playMml);
loopReplaces.looped = e2e.phrase(loopReplaces.loopMml);
e2e.assert(!loopReplaces.looped.pitches.some(p => loopReplaces.played.pitches.includes(p)),
    'Test phrases must not share pitches');
repl.test.loopTrace(true);
loopReplaces.looped.pitches;

// %% an ordinary clip on a second, plugin-less track, and a tutorial play
{
    uapmd.sequencer.addTrack();
    const otherTrack = uapmd.sequencer.getTrackInfos().length - 1;
    uapmd.timeline.createEmptyMidiClip(otherTrack, 0, 480, 120);
    const demo = include('builtin/lib/tutorial.js');
    demo.init({instanceId: e2e.instrument(), preset: 'Init Saw', bpm: 120});
    demo.play(loopReplaces.playMml);
    e2e.wait(400);
}

// %% loop entry while the ordinary clip plays
{
    const loopTrack = 0;
    const loop = include('builtin/lib/loop.js');
    loop.playLoopParts({test: loop.prepareLoopPart(loopTrack, loopReplaces.loopMml)}, 120);
    loopReplaces.peak = e2e.observe(5000).peak;
}

// %% only the loop is left
loopReplaces.result = (() => {
    const {looped, peak} = loopReplaces;
    const remaining = uapmd.sequencer.getTrackInfos()
        .map(track => [track.trackIndex, uapmd.timeline.getClips(track.trackIndex).length])
        .filter(([, count]) => count > 0);
    const trace = repl.test.loopTrace();
    const ons = trace.events.filter(e => e.on).map(e => e.note);
    // The ordinary phrase may sound before the loop starts; never after.
    const start = ons.findIndex(note => looped.pitches.includes(note));
    e2e.assert(start >= 0, `Loop never reached plugin: ${JSON.stringify(ons)}`);
    const after = ons.slice(start);
    e2e.assert(!remaining.length && after.length >= 8 && after.every(note => looped.pitches.includes(note)),
        `Ordinary clips survived loop entry: clips per track ${JSON.stringify(remaining)}, note-ons ${JSON.stringify(ons)}`);
    e2e.assert(peak > 0.001, 'Loop produced no output signal');
    return {noteOns: ons, loopStart: start, overflow: trace.overflow, peak};
})();

// %% stop
loopReplaces.result;
