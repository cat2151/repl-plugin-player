// %% @text name_try_parameter_ui
// @text demo_reset
{
    const instrumentTrack = 2;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    const synth = demo.prepareInstrument({format:'CLAP', plugin:'Six Sines', track:instrumentTrack, levelsOnly:true});
    const mml = 'l4 r c r c r c';
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    const loopNext = {main:prepareLoopPart(synth.trackIndex, mml)};
    playLoopParts(loopNext, 120);
    const parameter = uapmd.instance(synth.instanceId).getParameters().find(p => !p.isReadonly && !p.isDiscrete && /volume|level|gain/i.test(p.name));
    if (!parameter) throw new Error(repl.text('ui_snippet_missing'));
    repl.ui.show({id:'surge-parameters', title:'Six Sines', widgets:[
        {id:'parameter', type:'slider', label:parameter.name, binding:{instanceId:synth.instanceId, parameterId:parameter.id}}
    ]});
    synth;
}
