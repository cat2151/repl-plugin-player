// An immediate NoteOn from a Run sustains across a Run with a syntax error until the case's own NoteOff.

// %% silent baseline
e2e.note.baseline();

// %% NoteOn
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);

// %% sounds right after its Run
e2e.note.sounding();

// %% error - a syntax error
const = ;

// %% a Run after it
globalThis.afterError = true;

// %% sustains past one second
e2e.note.sustained();

// %% explicit NoteOff decays
uapmd.sequencer.sendNoteOff(e2e.instrument(), 60);
e2e.note.released();

// %% stop
e2e.note.peaks;
