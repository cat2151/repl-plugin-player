// The first Run after Ready, with no engine enable before it, sounds its NoteOn;
// it sustains past one second until the case's own NoteOff.

// %% NoteOn
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);

// %% sounds right after its Run
e2e.note.sounding();

// %% sustains past one second
e2e.note.sustained();

// %% explicit NoteOff decays
uapmd.sequencer.sendNoteOff(e2e.instrument(), 60);
e2e.note.released();

// %% stop
e2e.note.peaks;
