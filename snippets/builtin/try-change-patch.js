// %% @text name_try_change_patch
{
    include('builtin/lib/evolution-mode.js').end();
    const loopTrack = 0;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    const loopNext = Object.assign({}, globalThis.loopParts || {});
    loopNext.main = prepareLoopPart(loopTrack, ">cg<cg");
    playLoopParts(loopNext, 120);
}
// %% @text change_patch_cell
{
    const loopTrack = 0;
    include('builtin/lib/patch.js').change({track:loopTrack});
}
