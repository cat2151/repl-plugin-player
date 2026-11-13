// %% @text name_try_effect_ui
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
    const synth = demo.prepareInstrument({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', track:loopTrack});
    // @text demo_delay
    const fx = demo.prepareEffect(loopTrack, 'CLAP', 'org.surge-synth-team.surge-xt-fx');
    fx.showUI();
    uapmd.audio.setEngineEnabled(true);

    synth;
}
