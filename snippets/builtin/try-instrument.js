// %% @text name_try_instrument
// @text demo_reset
(() => {
    var demo = include('builtin/lib/tutorial.js');
    demo.reset();
    var synth = demo.prepareInstrument('tutorialInstrument', 'CLAP', 'org.baconpaul.six-sines', false);
    var mml = 'r c r c r c';
    var clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    return {...synth, ...clip};
})();
