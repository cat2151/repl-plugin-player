// %% @text name_try_delay
// @text demo_reset
(() => {
    var demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    var synth = demo.preparePreset(globalThis.surgeId, 'Init Saw');
    // @text demo_delay
    var fx = demo.prepareEffect(synth.trackIndex, 'CLAP', 'org.surge-synth-team.surge-xt-fx');
    var mml = 'l16 r >cegb<cegb';
    var clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    return {...synth, ...clip};
})();
