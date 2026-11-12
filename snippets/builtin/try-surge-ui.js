// %% @text name_try_surge_ui
// @text demo_reset
{
    const loopTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();

    const loopNext = Object.assign({}, globalThis.loopParts || {});
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    loopNext.main = prepareLoopPart(loopTrack, ">cg<cg");
    playLoopParts(loopNext, 120);

    // @text demo_levels
    const synth = demo.preparePreset(include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', track:loopTrack}).instanceId, 'Init Saw');
    uapmd.instance(synth.instanceId).showUI();
    synth;
}
