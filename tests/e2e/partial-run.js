// `partial-run`: the editor text that `src/execution_e2e.rs` runs piece by
// piece, by cell, selection and line, as the GUI does. Only the pieces it
// picks may run; every other line here throws or would change the counts.
throw new Error('outside cell');
// %% setup
var partialValue = 40;
// %% edit
partialValue += 2;
// %% neighbor
throw new Error('neighbor');
// %% selection
throw new Error('before');
partialValue += 1;
throw new Error('after');
// %% value
partialValue
// %% failing
partialValue = 44; throw new Error('expected fragment error');
// %% after failing
partialValue = -1;
// %% line
partialValue += 1;
throw new Error('next line');
// %% track and reset
var e2eTrack = uapmd.sequencer.getTrackInfos()[0];
if (!e2eTrack) throw new Error('E2E: no track holds the target plugin');
var e2eTrackId = e2eTrack.trackId;
uapmd.sequencer.stopPlayback();
uapmd.sequencer.sendNoteOff(include('builtin/lib/plugins.js').instrumentOn(0), 60);
uapmd.sequencer.jumpPlayback(0);
uapmd.timeline.setTempo(120);
for (var e2eClip of uapmd.timeline.getClips(e2eTrackId)) {
    if (e2eClip.clipType === 'midi' && !uapmd.timeline.removeClip(e2eClip.clipId))
        throw new Error('E2E: could not remove clip ' + e2eClip.clipId);
}

// %% one note: pitch 60 from beat 1 to beat 3
var e2eClipId = uapmd.timeline.createEmptyMidiClip(e2eTrackId, 0, 480, 120);
if (!e2eClipId) throw new Error('E2E: clip creation failed');
var e2eTpqn = uapmd.timeline.getClipUmpEvents(e2eClipId).tickResolution;
if (!uapmd.timeline.addUmpEvent(e2eClipId, e2eTpqn, [0x40903c00, 0x7fff0000])
    || !uapmd.timeline.addUmpEvent(e2eClipId, 3 * e2eTpqn, [0x40803c00, 0]))
    throw new Error('E2E: note events rejected');

// %% play
uapmd.sequencer.jumpPlayback(0);
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();
