// %% @text name_try_delay
// @text demo_reset
{
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    const synth = demo.preparePreset(include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', track:synthTrack}).instanceId, 'Init Saw');
    // @text demo_delay
    const fx = demo.prepareEffect(synthTrack, 'CLAP', 'org.surge-synth-team.surge-xt-fx');
    const mml = 'l16 r >cegb<cegb';
    const clip = demo.replaceMml(synthTrack, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    ({...synth, ...clip});
}
