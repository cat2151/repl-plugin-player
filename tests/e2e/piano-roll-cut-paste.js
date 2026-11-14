// `piano-roll-cut-paste`: cutting c from a c-f-g clip and pasting it after g
// moves it, in the piano roll and in what reaches the plugin. The view checks
// are Rust (`src/piano_roll_e2e.rs`); it runs these cells by title.
// Test-owned phrases: editable snippet contents are never expectations.

// %% a clip of c f g
{
    const synthTrack = 0;
    for (const clip of uapmd.timeline.getClips(e2e.trackId(synthTrack)))
        e2e.assert(uapmd.timeline.removeClip(clip.clipId), `Could not remove clip ${clip.clipId}`);
    uapmd.timeline.setTempo(120);
    const path = getGeneratedMidiPath();
    mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('r cfg'), path);
    const added = uapmd.timeline.addMidiClip(e2e.trackId(synthTrack), 0, path);
    e2e.assert(added.success, `Clip not added: ${added.error}`);
    globalThis.cutPaste = {clipId: added.clipId};
    cutPaste.clipId;
}

// %% cut c
{
    const c = uapmd.timeline.getClipNotes(cutPaste.clipId).notes.filter(n => n.note === 60);
    e2e.assert(c.length === 1, `Expected one c: ${JSON.stringify(c)}`);
    const cut = repl.pianoRoll.cut(cutPaste.clipId, [c[0].noteId]);
    e2e.assert(cut === 1, `cut returned ${cut}`);
    cut;
}

// %% paste c after g
{
    const g = uapmd.timeline.getClipNotes(cutPaste.clipId).notes.find(n => n.note === 67);
    const pasted = repl.pianoRoll.paste(cutPaste.clipId, g.endTick);
    const notes = uapmd.timeline.getClipNotes(cutPaste.clipId).notes.filter(n => pasted.includes(n.noteId));
    e2e.assert(notes.length === 1 && notes[0].note === 60 && notes[0].startTick === g.endTick,
        `Pasted at ${g.endTick}: ${JSON.stringify(notes)}`);
    notes[0].startTick;
}

// %% plays f g c
{
    repl.test.loopTrace(true);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    const ons = () => e2e.trace().events.filter(e => e.on).map(e => e.note);
    e2e.waitFor(() => ons().length >= 3, 4000);
    e2e.assert(e2e.same(ons().slice(0, 3), [65, 67, 60]), `Plugin input Note Ons: ${JSON.stringify(ons())}`);
    ons();
}

// %% exact ticks at 97 BPM
{
    const synthTrack = 0;
    const clipId = uapmd.timeline.createEmptyMidiClip(e2e.trackId(synthTrack), 0, 480, 97);
    e2e.assert(clipId, 'Clip creation failed');
    // c then e, a quarter note each.
    for (const [tick, words] of [[0, [0x40903c00, 0x7fff0000]], [480, [0x40803c00, 0]],
                                 [480, [0x40904000, 0x7fff0000]], [960, [0x40804000, 0]]])
        e2e.assert(uapmd.timeline.addUmpEvent(clipId, tick, words), 'Note events rejected');
    e2e.assert(uapmd.timeline.getClipNotes(clipId).tempo === 97, 'The clip tempo is not 97');
    const e = uapmd.timeline.getClipNotes(clipId).notes.find(n => n.note === 64);
    repl.pianoRoll.cut(clipId, [e.noteId]);
    // Ticks whose seconds at 97 BPM are not exact; the pasted e notes do not overlap.
    const ticks = [1, 1234, 2345, 4801];
    const starts = ticks.map(tick => {
        const [id] = repl.pianoRoll.paste(clipId, tick);
        return uapmd.timeline.getClipNotes(clipId).notes.find(n => n.noteId === id).startTick;
    });
    e2e.assert(e2e.same(starts, ticks), `Pasted at ${JSON.stringify(ticks)}, starts ${JSON.stringify(starts)}`);
    cutPaste.tempoClip = clipId;
    starts;
}

// %% paste into a missing clip
repl.pianoRoll.paste('no-such-clip', 0);

// %% paste from an empty clipboard
{
    const synthTrack = 0;
    const clipId = uapmd.timeline.createEmptyMidiClip(e2e.trackId(synthTrack), 0, 480, 120);
    e2e.assert(uapmd.timeline.addUmpEvent(clipId, 0, [0x40903c00, 0x7fff0000])
        && uapmd.timeline.addUmpEvent(clipId, 480, [0x40803c00, 0]), 'Note events rejected');
    repl.pianoRoll.paste(clipId, 0);
}

// %% a cell after the errors
repl.pianoRoll.paste(cutPaste.tempoClip, 0).length;
