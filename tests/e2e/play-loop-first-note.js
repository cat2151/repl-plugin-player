// Pressing the key `[scripts]` gives builtin/play-loop.js while the transport plays
// sounds the clip's first note. Test-owned clip: editable snippet contents are never expectations.

// %% init
// The app's own startup, which binds the keys of `[scripts]`.
include('builtin/init.js')

// %% blocks advance while the engine runs
// play-loop.js waits on this counter, the one repl.test.observe() reports as `blocks`.
{
    const blocks = () => __remidy_audio_worker_diagnostics().timingCounters.realtimeBlocks;
    uapmd.audio.setEngineEnabled(true);
    e2e.assert(e2e.waitFor(() => repl.audio.isProcessing(), 5000), 'Engine did not start');
    const first = blocks();
    const observed = repl.test.observe().blocks;
    e2e.wait(200);
    const last = blocks();
    const result = {first, observed, last};
    e2e.assert(last - first >= 1, `Blocks did not advance: ${JSON.stringify(result)}`);
    e2e.assert(observed >= first && observed <= last,
        `repl.test.observe() counts other blocks: ${JSON.stringify(result)}`);
    result;
}

// %% easy
include('builtin/easy-init.js')

// %% helpers
globalThis.firstNote = (() => {
    const key = (Object.entries(repl.keys.config().scripts)
        .find(([, path]) => path === 'builtin/play-loop.js') ?? [''])[0];
    e2e.assert(key !== '', 'keybinds.toml binds no key to builtin/play-loop.js');
    const results = {key};

    // Presses the key and checks the output 50-250 ms after the press, where the clip's first
    // note sounds and the note before the press has decayed.
    function press(label) {
        const t0 = Date.now();
        __replRunKey(key);
        e2e.assert(uapmd.timeline.getState().isPlaying, `${key} did not play (${label})`);
        let peak = 0;
        while (Date.now() - t0 < 250) {
            if (Date.now() - t0 >= 50) peak = Math.max(peak, repl.test.observe().peak);
            repl.processEvents();
        }
        (results[label] ??= []).push(+peak.toFixed(3));
        e2e.assert(peak > 1e-3, `The first note was silent: ${JSON.stringify(results)}`);
    }

    return {press, results};
})();

// %% after play
for (let i = 0; i < 3; i++) {
    play("cde");
    e2e.wait(2500);
    firstNote.press('after play');
}

// %% while looping
for (let i = 0; i < 3; i++) {
    e2e.wait(600);
    firstNote.press('while looping');
}

// %% stop
uapmd.sequencer.stopPlayback();
firstNote.results;
