// %% @text name_try_mml
// @text demo_reset
{
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    const synth = demo.preparePreset(include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', track:synthTrack}).instanceId, 'Init Saw');
    const mml = 'r cfg';
    const clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    ({...synth, ...clip});
}
