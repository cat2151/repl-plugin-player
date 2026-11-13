// %% @text name_try_preset
// @text demo_reset
{
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    const synth = demo.prepareInstrument({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Basses/Attacky.fxp', track:synthTrack});
    const mml = 'l4 r c r c r c';
    const clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    ({...synth, ...clip});
}
