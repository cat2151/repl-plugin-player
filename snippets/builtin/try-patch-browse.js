// %% @text name_try_patch_browse
{
    include('builtin/lib/evolution-mode.js').end();
    const loopTrack = 0;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    const loopNext = Object.assign({}, globalThis.loopParts || {});
    loopNext.main = prepareLoopPart(loopTrack, ">cg<cg");
    playLoopParts(loopNext, 120);
}
// %% @text patch_browse_cell
{
    const loopTrack = 0;
    include('builtin/lib/patch.js').browse({track:loopTrack});
}
