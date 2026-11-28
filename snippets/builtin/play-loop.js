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
    if (uapmd.timeline.getState().isPlaying) {
        // A note starting right after the flush is silenced (ADR 0033), so the restart waits
        // for one audio block after the stop.
        const blocks = () => __remidy_audio_worker_diagnostics().timingCounters.realtimeBlocks;
        uapmd.sequencer.stopPlayback();
        const stopped = blocks();
        const deadline = Date.now() + 1000;
        while (blocks() === stopped) {
            if (Date.now() > deadline) throw new Error('play-loop: no audio block for 1 s after stop');
            repl.processEvents();
        }
    }
    uapmd.sequencer.startPlayback();
}
