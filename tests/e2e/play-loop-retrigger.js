// Pressing the key `[scripts]` gives builtin/play-loop.js again and again restarts the loop
// from its head without stacking notes: no Note On meets a note that is still on, and the
// output peak does not grow from press to press. Each press runs `__replRunKey`, the JS a
// press of the key runs. Test-owned clip: editable snippet contents are never expectations.

// %% init
// The app's own startup, which binds the keys of `[scripts]`.
include('builtin/init.js')

// %% helpers
globalThis.retrigger = (() => {
    const key = (Object.entries(repl.keys.config().scripts)
        .find(([, path]) => path === 'builtin/play-loop.js') ?? [''])[0];
    e2e.assert(key !== '', 'keybinds.toml binds no key to builtin/play-loop.js');
    const results = {key};

    // Note Ons of the instrument that arrive while the same note is still on.
    function stacked(events, instrument) {
        const on = new Set();
        let count = 0;
        for (const e of events.filter(e => e.instance === instrument)) {
            if (!e.on) { on.delete(e.note); continue; }
            if (on.has(e.note)) count++;
            on.add(e.note);
        }
        return count;
    }

    // Presses the key `times` times, `ms` apart, starting stopped, and checks the trace.
    function presses(times, ms) {
        uapmd.sequencer.stopPlayback();
        uapmd.audio.setEngineEnabled(false);
        e2e.assert(e2e.waitFor(() => !repl.audio.isProcessing(), 5000), 'Engine did not stop');
        repl.test.loopTrace(true);
        const peaks = [];
        for (let i = 0; i < times; i++) {
            __replRunKey(key);
            e2e.assert(uapmd.timeline.getState().isPlaying, `${key} did not play (press ${i})`);
            peaks.push(e2e.observe(ms).peak);
        }
        const instrument = e2e.instrument();
        const events = e2e.trace().events;
        const ons = events.filter(e => e.on && e.instance === instrument).length;
        const result = {ms, peaks, ons, stacked: stacked(events, instrument)};
        results[`every ${ms} ms`] = result;
        e2e.assert(ons > 0, `No Note On reached the instrument: ${JSON.stringify(result)}`);
        e2e.assert(result.stacked === 0, `Notes stacked: ${JSON.stringify(result)}`);
        // The first press starts from silence, so its window holds the clip alone.
        e2e.assert(peaks[0] > 1e-3, `The first press was silent: ${JSON.stringify(result)}`);
        e2e.assert(peaks.every(peak => peak <= 1.5 * peaks[0]), `Peak grew: ${JSON.stringify(result)}`);
        return result;
    }

    return {presses, results};
})();

// %% clip
{
    const midi = include('builtin/lib/midi.js');
    const clipTrack = 0;
    for (const clip of uapmd.timeline.getClips(e2e.trackId(clipTrack))) uapmd.timeline.removeClip(clip.clipId);
    midi.addFile(clipTrack, midi.writeMml('l4 >cg<cg'));
}

// %% presses every 200 ms
retrigger.presses(6, 200);

// %% presses every 1 s
retrigger.presses(5, 1000);

// %% stop
retrigger.results;
