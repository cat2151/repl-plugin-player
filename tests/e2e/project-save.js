// Test-owned material; builtin snippet contents remain editable.
// %% prepare
{
    const melodyTrack = 0;
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'cde', 97);
    repl.track.setVolume(melodyTrack, 0.42);
}

// %% snapshot
(() => {
    const melodyTrack = 0;
    const trackId = include('builtin/lib/tracks.js').idOf(melodyTrack);
    return {
        volume: repl.track.volume(melodyTrack),
        clips: uapmd.timeline.getClips(trackId).map(clip => {
            const midi = uapmd.timeline.getClipUmpEvents(clip.clipId);
            return {positionSamples:clip.positionSamples, durationSamples:clip.durationSamples,
                tickResolution:midi.tickResolution, tempo:clip.tempo,
                // The SMF2 writer adds the clip name as Flex Data metadata text.
                // Compare performance events and timing, excluding that text.
                events:midi.events.filter(event => !((event.words[0] >>> 28) === 13
                    && ((event.words[0] >>> 8) & 255) === 1))
                    .map(event => ({tick:event.tick, words:event.words}))};
        }),
        plugin: repl.test.pluginState(include('builtin/lib/plugins.js').instrumentOn(melodyTrack))
    };
})();

// %% save
repl.project.saveArchive(projectPath);

// %% load
{
    const result = uapmd.project.load(projectPath);
    if (!result.success) throw new Error(result.error);
}

// %% invalid destination
(() => {
    try {
        repl.project.saveArchive(projectPath + '/missing/untitled.uapmdz');
    } catch (error) {
        return String(error).includes('project.saveArchive');
    }
    return false;
})();

// %% snippet smoke
include('builtin/save-project.js');

// %% prepare saved loop
{
    const melodyTrack = 0;
    const loop = include('builtin/lib/loop.js');
    loop.publishLoopParts({main:loop.prepareLoopPart(melodyTrack, 'l4 >cg<cg')}, 120);
}

// %% play snippet smoke
include('builtin/play.js');
uapmd.sequencer.stopPlayback();
uapmd.audio.setEngineEnabled(false);
