// The note monitor shows the notes the plugins actually received: immediate
// Note On/Off, notes kept through a failing Run, cleared by Stop, scheduled
// timeline notes, per-channel Note Off and channel CCs, and every instrument.
// Test-owned script: editable snippet contents are never expectations.

// %% nothing held at start
e2e.heldNotes([]);

// %% a Note On queued while the engine is off lights no key
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);
e2e.heldNotes([]);

// %% enabling the engine delivers it
uapmd.audio.setEngineEnabled(true);
e2e.heldNotes([60]);

// %% two more notes
uapmd.sequencer.sendNoteOn(e2e.instrument(), 64);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 72);
e2e.heldNotes([60, 64, 72]);

// %% one Note Off
uapmd.sequencer.sendNoteOff(e2e.instrument(), 60);
e2e.heldNotes([64, 72]);

// %% error - a Note On before an exception
uapmd.sequencer.sendNoteOn(e2e.instrument(), 61);
throw new Error('after note');

// %% the note from the failed Run is held
e2e.heldNotes([61, 64, 72]);

// %% stop - clears the held chord
e2e.heldNotes([]);

// %% a note after Stop
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);
e2e.heldNotes([60]);

// %% its Note Off
uapmd.sequencer.sendNoteOff(e2e.instrument(), 60);
e2e.heldNotes([]);

// %% scheduled delivery of a timeline clip rather than model prediction
e2e.timelineNote();
e2e.heldNotes([60]);
e2e.heldNotes([]);

// %% stop - same pitch on different channels, MIDI 1/2, velocity-zero and channel CCs
uapmd.timeline.removeClip(uapmd.timeline.getClips(e2e.trackId(0))[0].clipId);
uapmd.sequencer.jumpPlayback(0);
(() => {
    const clip = uapmd.timeline.createEmptyMidiClip(e2e.trackId(0), 0, 480, 120);
    const tpqn = uapmd.timeline.getClipUmpEvents(clip).tickResolution;
    [[1, [0x20903c7f]], [1, [0x20913c7f]], [1, [0x20903d7f]],
     [1, [0x40904800, 0x7fff0000]], [2, [0x20803c00]],
     [3, [0x20b17b00]], [4, [0x20903d00]], [4, [0x40b07800, 0]]]
        .forEach(e => uapmd.timeline.addUmpEvent(clip, e[0] * tpqn, e[1]));
})();
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% a Note Off on one channel keeps the other channel's note
e2e.heldNotes([60, 61, 72]);
e2e.wait(650);
e2e.assert(e2e.same(repl.test.notes(), [60, 61, 72]),
    "Note Off on one channel cleared another channel's note");
e2e.heldNotes([61, 72]);
e2e.heldNotes([]);

// %% stop - the keyboard shows every instrument, not only the prepared one
globalThis.otherId = uapmd.instancing.create('CLAP', 'org.baconpaul.six-sines');
if (otherId < 0) throw new Error('second instrument creation failed');
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();
uapmd.sequencer.sendNoteOn(otherId, 64);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 67);
e2e.heldNotes([64, 67]);

// %% the other instrument's Note Off
uapmd.sequencer.sendNoteOff(otherId, 64);
e2e.heldNotes([67]);

// %% stop
repl.test.notes();
