// Each attempt stops the engine (so the audio device stops), starts it again and holds one
// note for 400 ms, as `play()` does after its own stop. The device's first 150 ms after a
// start is where it ran dry. `scripts/first-note-dropout-test.py` records the device output
// while this case runs and looks for zero runs inside the notes; this case only plays them.
// Test-owned notes: editable snippet contents are never expectations.

// %% helpers
globalThis.firstNoteDropout = (() => {
    const instrument = e2e.instrument();
    for (const track of uapmd.sequencer.getTrackInfos())
        for (const node of track.nodes.filter(node => node.instanceId >= 0))
            uapmd.sequencer.setPluginBypassed(node.instanceId, false);

    function attempt() {
        uapmd.audio.setEngineEnabled(false);
        e2e.assert(e2e.waitFor(() => !repl.audio.isProcessing(), 5000), 'Engine did not stop');
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.sendNoteOn(instrument, 60);
        e2e.wait(400);
        uapmd.sequencer.sendNoteOff(instrument, 60);
        // Release plus the silence the recording needs to tell one attempt from the next.
        e2e.wait(1200);
        return 'played';
    }

    return {attempt};
})();

// %% attempt 0
firstNoteDropout.attempt();

// %% attempt 1
firstNoteDropout.attempt();

// %% attempt 2
firstNoteDropout.attempt();
