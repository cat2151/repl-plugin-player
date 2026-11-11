// %% @text name_try_loop_mml_multi_track
var {loopPhraseBeats, prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');

// Two independent tracks, one common cycle (120 BPM).
var loopTracks = uapmd.sequencer.getTrackInfos();
var loopMelodyTrack = loopTracks.find(t => t.nodes.some(n => n.instanceId === globalThis.surgeId));
if (!loopMelodyTrack) throw new Error(repl.text('snippet_00'));

// Reuse the bass instrument on subsequent runs; recreate it if its track was removed.
if (!loopTracks.some(t => t.nodes.some(n => n.instanceId === globalThis.loopBassId))) {
    var loopBass = uapmd.instancing.create('CLAP', 'org.surge-synth-team.surge-xt', -1);
    if (loopBass < 0) throw new Error(repl.text('snippet_02'));
    globalThis.loopBassId = loopBass;
}
var loopBassTrack = include('builtin/lib/plugins.js').trackFor(globalThis.loopBassId, 'snippet_02');

// Edit each MML and re-run: publish both parts together at the next feasible boundary.
var loopNext = {
    main: prepareLoopPart(loopMelodyTrack.trackIndex, "Isus4 I"),
    bass: prepareLoopPart(loopBassTrack.trackIndex, ">c1c1")
};
publishLoopParts(loopNext, 120);
uapmd.sequencer.setPluginBypassed(globalThis.surgeId, false);
uapmd.sequencer.setPluginBypassed(globalThis.loopBassId, false);
uapmd.audio.setEngineEnabled(true);
if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
