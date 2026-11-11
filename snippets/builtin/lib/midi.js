(() => {
    function writeMml(mml) {
        var bytes = mmlabc_to_smf.mml_to_smf_bytes(mml);
        var path = getGeneratedMidiPath();
        mmlabc_to_smf.pass4_midi.save_midi_file(bytes, path);
        return path;
    }

    function addFile(trackIndex, path) {
        var added = uapmd.timeline.addMidiClip(trackIndex, 0, path);
        if (!added.success) throw new Error(added.error);
        return added;
    }

    // note: {bpm, pitch, velocity (16-bit), start and length in beats}
    function addNoteClip(trackIndex, note, errors = ['snippet_06', 'snippet_07', 'snippet_08']) {
        var clipId = uapmd.timeline.createEmptyMidiClip(trackIndex, 0, 480, note.bpm);
        if (clipId < 0) throw new Error(repl.text(errors[0]));
        // @text comment_25
        var tickResolution = uapmd.timeline.getClipUmpEvents(trackIndex, clipId).tickResolution;
        // @text comment_26
        if (!uapmd.timeline.addUmpEvent(trackIndex, clipId, tickResolution * note.start,
            [0x40900000 + note.pitch * 0x100, note.velocity * 0x10000]))
            throw new Error(repl.text(errors[1]));
        if (!uapmd.timeline.addUmpEvent(trackIndex, clipId, tickResolution * (note.start + note.length),
            [0x40800000 + note.pitch * 0x100, 0]))
            throw new Error(repl.text(errors[2]));
        return {clipId, tickResolution};
    }

    return {writeMml, addFile, addNoteClip};
})();
