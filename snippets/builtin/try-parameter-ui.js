// %% @text name_try_parameter_ui
// @text demo_reset
(() => {
    var demo = include('builtin/lib/tutorial.js');
    demo.reset();
    var synth = demo.prepareInstrument('tutorialParameterInstrument', 'CLAP', 'org.baconpaul.six-sines', true);
    var mml = 'l4 r c r c r c';
    var {prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');
    var loopNext = {main:prepareLoopPart(synth.trackIndex, mml)};
    publishLoopParts(loopNext, 120);
    var parameter = uapmd.instance(synth.instanceId).getParameters().find(p => !p.isReadonly && !p.isDiscrete && /volume|level|gain/i.test(p.name));
    if (!parameter) throw new Error(repl.text('ui_snippet_missing'));
    repl.ui.show({id:'surge-parameters', title:'Six Sines', widgets:[
        {id:'parameter', type:'slider', label:parameter.name, binding:{instanceId:synth.instanceId, parameterId:parameter.id}}
    ]});
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    return synth;
})();
