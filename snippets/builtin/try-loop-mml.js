// %% @text name_try_loop_mml
var {loopPhraseBeats, prepareLoopPart, publishLoopParts} = include('builtin/lib/loop.js');

var loopTrack = include('builtin/lib/plugins.js').trackFor(globalThis.surgeId, 'snippet_00');
// Copy, prepare, then publish the complete set. Re-running "main" replaces it.
var loopNext = Object.assign({}, globalThis.loopParts || {});
loopNext.main = prepareLoopPart(loopTrack.trackIndex, "cegb");
publishLoopParts(loopNext, 120);
uapmd.sequencer.setPluginBypassed(globalThis.surgeId, false);
uapmd.audio.setEngineEnabled(true);
if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
