// %% @text name_try_chords
// @text demo_reset
{
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    const synth = demo.prepareInstrument({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', track:synthTrack});
    const mml = 'r ' + cmrt_chord.resolve_chord_or_mml('Isus4-I').mml;
    const clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    ({...synth, ...clip});
}
