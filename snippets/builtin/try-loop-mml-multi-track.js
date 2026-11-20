// %% @text name_try_loop_mml_multi_track
{
    include('builtin/lib/evolve-mode.js').end();
    // Two independent tracks, one common cycle (120 BPM).
    const melodyTrack = 0;
    const bassTrack = 1;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');

    // The bass track keeps its instrument; an empty one gets this plugin.
    include('builtin/lib/plugins.js').ensureInstrument(bassTrack, {format:'CLAP', plugin:'Surge XT'});

    // Edit each MML and re-run: update the ordinary clips immediately through UAPMD.
    const loopNext = {
        main: prepareLoopPart(melodyTrack, "Isus4 I"),
        bass: prepareLoopPart(bassTrack, ">c1c1")
    };
    playLoopParts(loopNext, 120);
}
