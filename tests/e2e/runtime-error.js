// An immediate NoteOn from a Run sustains across a Run that throws until the case's own NoteOff.

// %% silent baseline
e2e.note.baseline();

// %% NoteOn
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);

// %% sounds right after its Run
e2e.note.sounding();

// %% error - a throw after a side effect
globalThis.errorSideEffect = 42; throw new Error('expected E2E error')

// %% the side effect before the throw stays
e2e.assert(globalThis.errorSideEffect === 42, 'Runtime error lost preceding JS side effect');
globalThis.afterError = true;

// %% sustains past one second
e2e.note.sustained();

// %% explicit NoteOff decays
uapmd.sequencer.sendNoteOff(e2e.instrument(), 60);
e2e.note.released();

// %% stop
e2e.note.peaks;
