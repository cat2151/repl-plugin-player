// %% @text name_play_loop
{
    include('builtin/lib/evolve-mode.js').end();
    const clips = uapmd.sequencer.getTrackInfos()
        .flatMap(track => uapmd.timeline.getClips(track.trackId));
    const endSamples = clips.reduce((end, clip) =>
        Math.max(end, clip.positionSamples + clip.durationSamples), 0);
    const emptyTimelineSamples = Math.round(
        uapmd.sequencer.getSampleRate() * 60 / uapmd.timeline.getState().tempo * 4);
    repl.timeline.setLoop(0, Math.max(1, Math.ceil(endSamples || emptyTimelineSamples)));
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}
