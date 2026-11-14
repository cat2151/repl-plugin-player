// %% @text name_try_change_effect
{
    const loopTrack = 0;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    const loopNext = Object.assign({}, globalThis.loopParts || {});
    loopNext.main = prepareLoopPart(loopTrack, ">cg<cg");
    playLoopParts(loopNext, 120);
}
// %% @text change_effect_cell
{
    const loopTrack = 0;
    include('builtin/lib/effect.js').change({track:loopTrack});
}
