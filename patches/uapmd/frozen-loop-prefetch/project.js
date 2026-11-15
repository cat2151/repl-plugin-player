// %% load and freeze
{
    uapmd.audio.setEngineEnabled(false);
    __replCall('project.load', {filepath:prefetchProject});
    globalThis.prefetchTracks = uapmd.sequencer.getTrackInfos()
        .filter(t => uapmd.timeline.getClips(t.trackId).length);
    for (const track of prefetchTracks)
        uapmd.sequencer.setTrackFreezePolicy(track.trackId, 'on');
}

// %% ready
(() => {
    const states = prefetchTracks.map(t => uapmd.sequencer.getTrackFreezeState(t.trackId));
    for (const state of states)
        if (state.runtimeState === 'error') throw new Error(JSON.stringify(state));
    return states.length > 0 && states.every(s => s.runtimeState === 'frozen');
})()

// %% prepare loop
(() => {
    const clips = prefetchTracks.flatMap(t => uapmd.timeline.getClips(t.trackId));
    const end = Math.ceil(Math.max(...clips.map(c => c.positionSamples + c.durationSamples)));
    __replCall('timeline.setLoop', {startSamples:0, endSamples:end});
    const track = uapmd.sequencer.getTrackInfos()[prefetchTrackIndex];
    const instrument = track.nodes.find(n => n.instanceId >= 0);
    if (!instrument) throw new Error('Missing scope instrument');
    __replCall('scope.setTarget', {instanceId:instrument.instanceId});
    return {frames:end, rate:uapmd.sequencer.getSampleRate(), tracks:prefetchTracks.length};
})()

// %% start
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% stop
uapmd.sequencer.stopPlayback();
uapmd.audio.setEngineEnabled(false);
