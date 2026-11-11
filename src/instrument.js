// Keep original receivers, return values and exceptions. Observation is control-thread only.
globalThis.__replEvents = [];
for (const [object, key, api] of [
    [uapmd.audio, 'setEngineEnabled', 'uapmd.audio.setEngineEnabled'],
    [uapmd.sequencer, 'sendNoteOn', 'uapmd.sequencer.sendNoteOn'],
    [uapmd.sequencer, 'sendNoteOff', 'uapmd.sequencer.sendNoteOff'],
    [uapmd.sequencer, 'stopPlayback', 'uapmd.sequencer.stopPlayback'],
    [uapmd.sequencer, 'startPlayback', 'uapmd.sequencer.startPlayback'],
    [uapmd.sequencer, 'jumpPlayback', 'uapmd.sequencer.jumpPlayback'],
    [uapmd.timeline, 'setTempo', 'uapmd.timeline.setTempo'],
    [uapmd.timeline, 'createEmptyMidiClip', 'uapmd.timeline.createEmptyMidiClip'],
    [uapmd.timeline, 'addMidiClip', 'uapmd.timeline.addMidiClip'],
    [uapmd.timeline, 'removeClip', 'uapmd.timeline.removeClip'],
    [uapmd.timeline, 'addUmpEvent', 'uapmd.timeline.addUmpEvent'],
    [uapmd.timeline, 'removeUmpEvent', 'uapmd.timeline.removeUmpEvent']
]) {
    const original = object[key];
    object[key] = function (...args) {
        try {
            const result = Reflect.apply(original, this, args);
            __replEvents.push({api, args, result: result === undefined ? 'undefined' : result});
            return result;
        } catch (error) {
            __replEvents.push({api, args, exception: String(error)});
            throw error;
        }
    };
}
