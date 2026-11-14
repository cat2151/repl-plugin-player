// Immediate clip updates use UAPMD, without a reserved future loop boundary.
// %% prepare a long loop
globalThis.liveLoop = {track:0, old:'l8 cccccccccccccccc', next:'l8 gggggggggggggggg'};
liveLoop.lib = include('builtin/lib/loop.js');
repl.test.loopTrace(true);
liveLoop.lib.playLoopParts({main:liveLoop.lib.prepareLoopPart(liveLoop.track,liveLoop.old)},120);
e2e.wait(400);
liveLoop.oldPitch = e2e.phrase(liveLoop.old).pitches[0];
liveLoop.nextPitch = e2e.phrase(liveLoop.next).pitches[0];
e2e.assert(e2e.ons(e2e.trace().events).some(([, p])=>p===liveLoop.oldPitch), 'Initial clip did not play');

// %% replace during the first pass
liveLoop.frame = (() => { const events=e2e.trace().events; return events[events.length-1].frame; })();
liveLoop.lib.playLoopParts({main:liveLoop.lib.prepareLoopPart(liveLoop.track,liveLoop.next)},120);
e2e.assert(uapmd.timeline.getState().isPlaying, 'Clip update stopped the transport');
liveLoop.updated = e2e.waitFor(() => e2e.ons(e2e.trace().events).find(([frame,p]) =>
    frame > liveLoop.frame && p === liveLoop.nextPitch),900);
e2e.assert(liveLoop.updated && liveLoop.updated[0] < e2e.phrase(liveLoop.old).period,
    'Update waited for a loop boundary');
e2e.assert(e2e.clips(liveLoop.track).length === 1, 'Old and new clips accumulated');

// %% shorter period, rests and another update
liveLoop.lib.playLoopParts({main:liveLoop.lib.prepareLoopPart(liveLoop.track,'l8 err')},120);
{
    const sr = uapmd.sequencer.getSampleRate();
    const state = uapmd.timeline.getState();
    e2e.assert(state.loopEndSamples === Math.round(sr*0.75), 'Range did not update immediately');
    const phrase = e2e.phrase('l8 err');
    const start = e2e.trace().events.length;
    const peak = e2e.observe(2600).peak;
    const ons = e2e.ons(e2e.trace().events.slice(start));
    e2e.assert(peak > 0.001 && ons.length >= 3 && ons.every(([,p])=>p===phrase.pitches[0]),
        'Updated ordinary clip did not repeat');
    ons.forEach(([frame],i) => e2e.assert(i===0 || Math.abs(frame-ons[i-1][0]-phrase.period)<=2,
        `Shortened period drift: ${JSON.stringify(ons)}`));
}

// %% tempo changes use standard master MIDI clips
liveLoop.lib.playLoopParts({main:liveLoop.lib.prepareLoopPart(liveLoop.track,'l8 err')},97);
{
    e2e.wait(80);
    const sr = uapmd.sequencer.getSampleRate();
    const state = uapmd.timeline.getState();
    e2e.assert(Math.abs(state.tempo-97) < 0.001 && state.loopEndSamples === Math.round(1.5*60/97*sr),
        `Tempo and range disagree: ${JSON.stringify(state)}`);
    const masters = uapmd.timeline.getClips(uapmd.timeline.getMasterTrackId());
    e2e.assert(masters.length === 1, `Obsolete master clips accumulated: ${JSON.stringify(masters)}`);
    const start = e2e.trace().events.length;
    e2e.wait(3100);
    const ons = e2e.ons(e2e.trace().events.slice(start));
    e2e.assert(ons.length >= 3 && ons.every(([frame],i) => i===0 ||
        Math.abs(frame-ons[i-1][0]-state.loopEndSamples)<=2),
        `Tempo update did not reach clip playback: ${JSON.stringify(ons)}`);
}

// %% invalid update preserves the clips and loop
liveLoop.before = JSON.stringify(e2e.clips(liveLoop.track));
liveLoop.period = uapmd.timeline.getState().loopEndSamples;
liveLoop.masters = JSON.stringify(uapmd.timeline.getClips(uapmd.timeline.getMasterTrackId()));

// %% error invalid MIDI import
liveLoop.lib.publishLoopParts({
    first:liveLoop.lib.prepareLoopPart(liveLoop.track,'eeee'),
    second:{trackIndex:1,filepath:'X:/missing-loop.mid',beats:4}
},120);

// %% existing content survives failed import
e2e.assert(JSON.stringify(e2e.clips(liveLoop.track)) === liveLoop.before &&
    JSON.stringify(uapmd.timeline.getClips(uapmd.timeline.getMasterTrackId())) === liveLoop.masters &&
    uapmd.timeline.getState().loopEndSamples === liveLoop.period && uapmd.timeline.getState().isPlaying,
    'Failed import changed the clips or transport');

// %% stop
e2e.stopped();
