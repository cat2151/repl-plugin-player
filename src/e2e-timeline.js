// %% track and reset
// Test-owned timeline script for the internal E2E cases; built-in snippets may change freely.
var e2eTrack = uapmd.sequencer.getTrackInfos()
    .find(t => t.nodes.some(n => n.instanceId === globalThis.surgeId));
if (!e2eTrack) throw new Error('E2E: no track holds the target plugin');
var e2eTrackIndex = e2eTrack.trackIndex;
// BEGIN RESET
uapmd.sequencer.stopPlayback();
uapmd.sequencer.sendNoteOff(globalThis.surgeId, 60);
uapmd.sequencer.jumpPlayback(0);
uapmd.timeline.setTempo(120);
for (var e2eClip of uapmd.timeline.getClips(e2eTrackIndex)) {
    if (e2eClip.clipType === 'midi' && !uapmd.timeline.removeClip(e2eTrackIndex, e2eClip.clipId))
        throw new Error('E2E: could not remove clip ' + e2eClip.clipId);
}
// END RESET

// %% one note: pitch 60 from beat 1 to beat 3
var e2eClipId = uapmd.timeline.createEmptyMidiClip(e2eTrackIndex, 0, 480, 120);
if (e2eClipId < 0) throw new Error('E2E: clip creation failed');
var e2eTpqn = uapmd.timeline.getClipUmpEvents(e2eTrackIndex, e2eClipId).tickResolution;
if (!uapmd.timeline.addUmpEvent(e2eTrackIndex, e2eClipId, e2eTpqn, [0x40903c00, 0x7fff0000])
    || !uapmd.timeline.addUmpEvent(e2eTrackIndex, e2eClipId, 3 * e2eTpqn, [0x40803c00, 0]))
    throw new Error('E2E: note events rejected');

// %% play
uapmd.sequencer.jumpPlayback(0);
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();
