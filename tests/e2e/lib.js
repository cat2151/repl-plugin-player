// Helpers for E2E cases, evaluated once before a case's first cell.
// Cell titles (`// %% <title>`) are split on spaces: a standalone word `init`, `stop` or
// `error` is a runner directive, while `stop,` or `error:` is not. Use other words
// (`initialize`, `a Run after the failure`) in titles that are not directives.
globalThis.e2e = (() => {
    // Keeps the host's main thread serviced while a cell waits. Each wait is one pass of
    // the control loop for the frame monitor, so a long call between waits is a stall.
    function wait(ms) {
        const end = Date.now() + ms;
        while (Date.now() < end) repl.processEvents();
        __replCall('frameMonitor.beat', {});
    }

    // The first truthy `probe()` within `ms`, else the last value it returned.
    function waitFor(probe, ms) {
        const end = Date.now() + ms;
        for (;;) {
            const value = probe();
            if (value || Date.now() >= end) return value;
            wait(50);
        }
    }

    function assert(condition, message) {
        if (!condition) throw new Error(message);
    }

    const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

    // The instrument on track 0, looked up the way cells do.
    const instrument = () => include('builtin/lib/plugins.js').instrumentOn(0);

    // uapmd's trackId of the track numbered `track`, looked up the way cells do.
    const trackId = track => include('builtin/lib/tracks.js').idOf(track);

    // The output peak over `ms`, sampled every 25 ms, and the last block count.
    // Every sample must come from a running engine.
    function observe(ms) {
        const end = Date.now() + ms;
        let peak = 0, blocks;
        do {
            const value = repl.test.observe();
            assert(value.finite && value.engineEnabled && value.dispatcherPlaying,
                `Invalid observation: ${JSON.stringify(value)}`);
            peak = Math.max(peak, value.peak);
            blocks = value.blocks;
            wait(25);
        } while (Date.now() < end);
        return {peak, blocks};
    }

    // Checks of one immediate note, one per cell; the case's own cells send
    // NoteOn and NoteOff. `peaks` collects the measured values.
    const note = (() => {
        const peaks = {};
        let first;
        return {
            peaks,
            baseline() {
                uapmd.audio.setEngineEnabled(true);
                const {peak} = observe(500);
                assert(peak <= 1e-6, `Baseline not silent: ${peak}`);
                return peaks.baseline = peak;
            },
            sounding(ms = 300) {
                first = observe(ms);
                assert(first.peak > 1e-3, `No note signal: ${first.peak}`);
                return peaks.first = first.peak;
            },
            // Past the removed one-second deadline, in a window whose blocks
            // advance, so analyser history cannot pass for sustain.
            sustained() {
                observe(1200);
                const held = observe(250);
                assert(held.peak > 1e-3, `Note did not sustain across Run/error/time: ${held.peak}`);
                assert(held.blocks > first.blocks, 'Realtime block counter did not advance');
                return peaks.held = held.peak;
            },
            released() {
                observe(500);
                const {peak} = observe(500);
                assert(peak <= 1e-5, `Explicit NoteOff did not decay: ${peak}`);
                return peaks.released = peak;
            }
        };
    })();

    // The plugin's held input notes become `expected` within four seconds.
    function heldNotes(expected) {
        const notes = waitFor(() => {
            const notes = repl.test.notes();
            return same(notes, expected) && notes;
        }, 4000);
        assert(notes, `Expected live notes ${JSON.stringify(expected)}, got ${JSON.stringify(repl.test.notes())}`);
        return notes;
    }

    // The track's clips, each MIDI clip with its UMP events.
    const clips = track => uapmd.timeline.getClips(trackId(track)).map(clip => ({
        ...clip,
        midi: clip.clipType === 'midi' ? uapmd.timeline.getClipUmpEvents(clip.clipId) : null
    }));

    // Notes paired from MIDI 1/2 UMP Note On/Off by group, channel and pitch.
    // A velocity-zero Note On ends a note; `end` stays undefined while held.
    function notes(events) {
        const result = [], held = new Map();
        for (const {tick, words} of events) {
            const word = words[0] >>> 0;
            const kind = word >>> 28, status = (word >>> 20) & 15;
            if ((kind !== 2 && kind !== 4) || (status !== 8 && status !== 9)) continue;
            const pitch = (word >>> 8) & 127;
            const key = `${(word >>> 24) & 15}/${(word >>> 16) & 15}/${pitch}`;
            const velocity = kind === 2 ? word & 127 : (words[1] >>> 16) & 65535;
            const queue = held.get(key) || [];
            held.set(key, queue);
            if (status === 9 && velocity > 0) {
                queue.push(result.length);
                result.push({pitch, start: tick, end: undefined});
            } else if (queue.length && result[queue[0]].start <= tick) {
                result[queue.shift()].end = tick;
            }
        }
        return result;
    }

    // One clip on track 0 holding pitch 60 from beat 1 to beat 3, then playback
    // from the start. `reset` first stops, silences and removes the MIDI clips.
    function timelineNote({reset = true} = {}) {
        const track = 0;
        if (reset) {
            uapmd.sequencer.stopPlayback();
            uapmd.sequencer.sendNoteOff(instrument(), 60);
            uapmd.sequencer.jumpPlayback(0);
            uapmd.timeline.setTempo(120);
            for (const clip of uapmd.timeline.getClips(trackId(track))) {
                if (clip.clipType === 'midi' && !uapmd.timeline.removeClip(clip.clipId))
                    throw new Error(`E2E: could not remove clip ${clip.clipId}`);
            }
        }
        const clip = uapmd.timeline.createEmptyMidiClip(trackId(track), 0, 480, 120);
        assert(clip, 'E2E: clip creation failed');
        const tpqn = uapmd.timeline.getClipUmpEvents(clip).tickResolution;
        assert(uapmd.timeline.addUmpEvent(clip, tpqn, [0x40903c00, 0x7fff0000])
            && uapmd.timeline.addUmpEvent(clip, 3 * tpqn, [0x40803c00, 0]),
            'E2E: note events rejected');
        uapmd.sequencer.jumpPlayback(0);
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.startPlayback();
        return clip;
    }

    // Over `ms`, the highest output peak and every pitch the plugin input held,
    // sampled every 25 ms; every sample must be finite.
    function heard(ms) {
        const end = Date.now() + ms;
        const pitches = new Set();
        let peak = 0;
        do {
            const value = repl.test.observe();
            assert(value.finite, `Non-finite output: ${JSON.stringify(value)}`);
            peak = Math.max(peak, value.peak);
            for (const pitch of repl.test.notes()) pitches.add(pitch);
            wait(25);
        } while (Date.now() < end);
        return {peak, pitches: [...pitches].sort((a, b) => a - b)};
    }

    // The converter's Note Ons for `mml` as the loop plays it at 120 BPM: pitches,
    // onset frames within one pass, and the frames of one pass. `pitch(i)` and
    // `at(i, period)` give the i-th Note On when the pass repeats every `period`.
    function phrase(mml) {
        const frames = beats => Math.round(beats * uapmd.sequencer.getSampleRate() / 2);
        const events = mmlabc_to_smf.pass3_events.ast_to_events(
            mmlabc_to_smf.pass2_ast.tokens_to_ast(mmlabc_to_smf.pass1_parser.parse_mml(
                cmrt_chord.resolve_chord_or_mml(mml).mml)), true)
            .filter(e => e.type === 'note_on');
        const pitches = events.map(e => e.note), onsets = events.map(e => frames(e.time / 480));
        return {
            pitches,
            onsets,
            period: frames(include('builtin/lib/loop.js').loopPhraseBeats(mml)),
            pitch: i => pitches[i % pitches.length],
            at: (i, period) => Math.floor(i / pitches.length) * period + onsets[i % pitches.length]
        };
    }

    // The plugin-input trace since the last `repl.test.loopTrace(true)`, which
    // must hold every event and no timing marker.
    function trace() {
        const value = repl.test.loopTrace();
        assert(value.overflow === 0 && value.programChanges === 0,
            'Plugin-input trace overflow or timing marker delivery');
        return value;
    }

    // [frame, note] of each Note On, of one instance when `instance` is given.
    const ons = (events, instance) => events
        .filter(e => e.on && (instance === undefined || e.instance === instance))
        .map(e => [e.frame, e.note]);

    // After a Stop: the transport, plugin input, engine and dispatcher stay
    // stopped, and no block runs for 250 ms.
    function stopped() {
        const observed = repl.test.observe();
        wait(250);
        const last = repl.test.observe();
        assert(uapmd.timeline.getState().isPlaying === false && repl.test.notes().length === 0
            && observed.engineEnabled === false && last.dispatcherPlaying === false
            && last.blocks === observed.blocks, 'Loop survived Stop');
        return last.blocks;
    }

    return {wait, waitFor, assert, same, instrument, trackId, observe, note, heldNotes, clips, notes, timelineNote, heard, phrase,
        trace, ons, stopped};
})();
