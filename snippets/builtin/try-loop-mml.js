// %% @text name_try_loop_mml
{
    const loopTrack = 0;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');

    // Copy, prepare, then publish the complete set. Re-running "main" replaces it.
    const loopNext = Object.assign({}, globalThis.loopParts || {});
    loopNext.main = prepareLoopPart(loopTrack, "cegb");
    playLoopParts(loopNext, 120);
}
