// Closing while a note sounds cleans up; the runner closes after the last cell.

// %% silent baseline
e2e.note.baseline();

// %% NoteOn
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);

// %% sounds before close
e2e.note.sounding(250);
