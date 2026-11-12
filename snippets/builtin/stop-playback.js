// @text comment_38
// @text comment_39
{
    const noteTrack = 0;
    uapmd.sequencer.stopPlayback();
    const instrument = include('builtin/lib/plugins.js').instrumentOn(noteTrack);
    if (instrument >= 0) uapmd.sequencer.sendNoteOff(instrument, 60);
}
