// %% @text name_try_surge_ui
// @text demo_reset
(() => {
    var demo = include('builtin/lib/tutorial.js');
    demo.reset();

    var loopTrack = include('builtin/lib/plugins.js').trackFor(globalThis.surgeId, 'snippet_00');
    var loopNext = Object.assign({}, globalThis.loopParts || {});
    var {loopPhraseBeats, prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');
    loopNext.main = prepareLoopPart(loopTrack.trackIndex, ">cg<cg");
    publishLoopParts(loopNext, 120);
    uapmd.audio.setEngineEnabled(true);
    if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();

    // @text demo_levels
    var synth = demo.preparePreset(globalThis.surgeId, 'Init Saw');
    uapmd.instance(synth.instanceId).showUI();
    return synth;
})();
