// After the tutorial `reset()` and a Stop, an ordinary clip at position 0
// delivers its first Note On. The loss depends on whether an audio block runs
// before startPlayback, so reset, Stop and play repeat three times.
// Test-owned phrase: editable snippet contents are never expectations.

// %% helpers
globalThis.resetFirstNote = (() => {
    const mml = 'l2 cde';
    const expected = e2e.phrase(mml);
    const results = [];

    // Replaces the track's clips with the phrase at position 0, then starts the engine and transport.
    function playClip() {
        for (const track of uapmd.sequencer.getTrackInfos())
            for (const node of track.nodes.filter(node => node.instanceId >= 0))
                uapmd.sequencer.setPluginBypassed(node.instanceId, false);
        const clipTrack = 0;
        for (const clip of uapmd.timeline.getClips(e2e.trackId(clipTrack))) uapmd.timeline.removeClip(clip.clipId);
        const path = getGeneratedMidiPath();
        mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes(mml), path);
        const clip = uapmd.timeline.addMidiClip(e2e.trackId(clipTrack), 0, path);
        if (!clip.success) throw new Error(clip.error);
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.startPlayback();
    }

    // Something is sounding when reset() stops it, as when the user resets mid-playback.
    function reset() {
        playClip();
        e2e.wait(600);
        include('builtin/lib/tutorial.js').reset();
    }

    // The next cell's title stops first: the GUI Stop after reset, with the transport already stopped.
    function playAfterStop(attempt) {
        repl.test.loopTrace(true);
        playClip();
        const last = expected.onsets[expected.onsets.length - 1];
        e2e.wait(last / uapmd.sequencer.getSampleRate() * 1000 + 800);
        const trace = repl.test.loopTrace();
        e2e.assert(trace.overflow === 0, 'Plugin-input trace overflow');
        const instrument = e2e.instrument();
        const notes = trace.events.filter(e => e.on && e.instance === instrument).map(e => [e.frame, e.note]);
        const pitches = notes.map(([, note]) => note);
        e2e.assert(pitches[0] === expected.pitches[0],
            `First note missing after reset (attempt ${attempt}): ${JSON.stringify(notes)}`);
        e2e.assert(e2e.same(pitches, expected.pitches),
            `Phrase after reset differs (attempt ${attempt}): ${JSON.stringify(notes)}`);
        results.push({attempt, noteOns: notes});
        return notes;
    }

    return {reset, playAfterStop, results};
})();

// %% reset while playing (attempt 0)
resetFirstNote.reset();

// %% stop - then play from position 0 (attempt 0)
resetFirstNote.playAfterStop(0);

// %% reset while playing (attempt 1)
resetFirstNote.reset();

// %% stop - then play from position 0 (attempt 1)
resetFirstNote.playAfterStop(1);

// %% reset while playing (attempt 2)
resetFirstNote.reset();

// %% stop - then play from position 0 (attempt 2)
resetFirstNote.playAfterStop(2);

// %% stop
resetFirstNote.results;
