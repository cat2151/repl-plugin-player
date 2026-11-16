// %% @text name_try_evolving_sequencer
{
    const instruments = ['Surge XT','Vaporizer2','TyrellN6','Six Sines'];
    include('builtin/lib/evolution-mode.js').enter(
        instruments.map(plugin=>({format:'CLAP',plugin})));
    const sequenceTracks = globalThis.evolutionPool.buffers.map(b=>b.track);
    const demo = include('builtin/lib/tutorial.js');
    demo.reset({keepEvolution:true});
    repl.sequence.clear();
    for (const track of uapmd.sequencer.getTrackInfos().filter(t=>globalThis.evolutionMode.trackIds.includes(t.trackId)))
        for (const clip of uapmd.timeline.getClips(track.trackId))
            if (!uapmd.timeline.removeClip(clip.clipId)) throw new Error('Could not remove clip');
    for (const track of sequenceTracks)
        uapmd.sequencer.setTrackFreezePolicy(include('builtin/lib/tracks.js').idOf(track), 'off');
    uapmd.audio.setEngineEnabled(true);
}
// %% @text evolving_sequence_edit
{
    // Re-run only this cell while playing: the native clock keeps its phase.
    include('builtin/lib/evolution.js').start({
        mode:'sequence', pool:globalThis.evolutionPool,
        sounds:{role:'chord', kind:'pad'},
        crossfade:{holdMs:5000, fadeMs:5000, everyMs:25},
        progression:'CM7-Dm7-Em7-FM7-G6-Am7',
        inversions:[0, 1, 2, 3], octaves:[-1, 0, 1], maxLeap:7,
        range:{min:60-12-7, max:72+12+7, low:60, high:71},
        bpm:72, beats:4, velocity:24000, everyMs:2000
    });
}
