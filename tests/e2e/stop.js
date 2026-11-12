// Stop silences a held note and leaves the engine and device stopped.

// %% silent baseline
e2e.note.baseline();

// %% NoteOn
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.sendNoteOn(e2e.instrument(), 60);

// %% sounds before Stop
e2e.note.sounding(250);

// %% stop - then the block counter stays
{
    const first = repl.test.observe();
    e2e.wait(200);
    const last = repl.test.observe();
    e2e.assert(!last.engineEnabled && !last.dispatcherPlaying && first.blocks === last.blocks,
        `Engine/device did not remain stopped: ${JSON.stringify([first, last])}`);
    last.blocks;
}
