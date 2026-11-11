// %% @text name_try_mml
// @text demo_reset
(() => {
    var demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    var synth = demo.preparePreset(globalThis.surgeId, 'Init Saw');
    var mml = 'r cfg';
    var clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    return {...synth, ...clip};
})();
