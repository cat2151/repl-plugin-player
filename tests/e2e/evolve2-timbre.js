// Patches that go into an instance without a plugin state (a cartridge program as
// SysEx, a preset through the plugin's own extension), prepared by patchJob as the
// evolve2 pool prepares them, make different outputs on the evolve2 instances; the
// first patch again makes nearly the same output, which bounds what noise alone does.
// The evolve host of evolving-sequencer takes the same files.
// Test-owned script: editable snippet contents are never read.

// %% helpers
globalThis.evolve2Timbre = (() => {
    const {assert, wait, waitFor} = e2e;
    const call = (name, args = {}) => __replCall('evolve2.' + name, args);
    const before = uapmd.sequencer.getTrackInfos().map(t => t.trackId);
    for (let i = 0; i < 2; ++i) {
        const job = uapmd.sequencer.addTrack();
        waitFor(() => uapmd.mutations.getJob(job.jobId).state !== 'pending', 30000);
        assert(uapmd.mutations.getJob(job.jobId).state !== 'failed', 'Track was not added');
    }
    const infos = uapmd.sequencer.getTrackInfos();
    const tracks = infos.map((t, index) => before.includes(t.trackId) ? -1 : index).filter(index => index >= 0);
    const trackIds = tracks.map(index => infos[index].trackId);

    // The state file patchJob prepares for the patch.
    function prepared(pluginId, patch) {
        const id = __replCall('patchJob.start', {format:'CLAP', pluginId, patch});
        const progress = waitFor(() => {
            const value = __replCall('patchJob.get', {jobId:id});
            return value.state !== 'pending' && value;
        }, 30000);
        assert(progress && progress.state === 'completed', 'Patch was not prepared: ' + JSON.stringify(progress));
        return {id, file:progress.file};
    }

    function load(slot, pluginId, patch) {
        const {id, file} = prepared(pluginId, patch);
        const request = call('submit', {slot, format:'CLAP', pluginId, file});
        __replCall('patchJob.release', {jobId:id});
        const outcomes = [];
        waitFor(() => {
            outcomes.push(...call('poll'));
            return outcomes.some(o => o.requestId === request);
        }, 60000);
        const outcome = outcomes.find(o => o.requestId === request);
        assert(outcome && !outcome.stale && outcome.error === '', 'Load did not settle: ' + JSON.stringify(outcomes));
    }

    // Whether `slot` alone is heard, at full gain; the other slot may be playing
    // silently before its fade in.
    const alone = slot => {
        const status = call('status');
        return status[slot].state === 3 && status[slot].gain === 1 && status[1 - slot].gain === 0;
    };

    // The master output's rms every 25 ms over two chords, once `slot` alone sounds.
    function profile(slot) {
        assert(waitFor(() => alone(slot), 20000), 'Slot ' + slot + ' did not sound alone: ' + JSON.stringify(call('status')));
        wait(300);
        const rms = [];
        const end = Date.now() + 2000;
        while (Date.now() < end) {
            const value = repl.test.observe();
            assert(value.finite, 'Invalid observation: ' + JSON.stringify(value));
            rms.push(value.rms);
            wait(25);
        }
        assert(alone(slot), 'Slot ' + slot + ' stopped sounding alone while it was measured');
        assert(Math.max(...rms) > 1e-3, 'No signal on slot ' + slot + ': ' + JSON.stringify(rms));
        return rms;
    }

    // As in patch-timbre: 0 for profiles holding the same levels, compared in order of size.
    function distance(a, b) {
        const n = Math.min(a.length, b.length);
        a = [...a].sort((x, y) => x - y);
        b = [...b].sort((x, y) => x - y);
        let apart = 0, scale = 0;
        for (let i = 0; i < n; i++) {
            apart += Math.abs(a[i] - b[i]);
            scale += Math.max(a[i], b[i]);
        }
        return apart / scale;
    }

    // A on slot 0 and B on slot 1 before playback, then A again on slot 0 once B is measured.
    function differ(pluginId, first, second) {
        call('open', {tracks, holdMs:5000, fadeMs:300});
        try {
            load(0, pluginId, first);
            load(1, pluginId, second);
            // A patch may finish loading on the plugin's main thread callbacks.
            wait(1500);
            repl.sequence.set({track:tracks[0], tracks, steps:[[60]], bpm:120, beats:2, gate:0.5, velocity:30000});
            uapmd.audio.setEngineEnabled(true);
            uapmd.sequencer.startPlayback();
            const a = profile(0);
            const b = profile(1);
            // Slot 1 keeps sounding until slot 0 is loaded again.
            assert(waitFor(() => call('status')[0].state === 0, 20000), 'Slot 0 was not retired: ' + JSON.stringify(call('status')));
            load(0, pluginId, first);
            const again = profile(0);
            const level = rms => Math.round(1e4 * rms.reduce((x, y) => x + y, 0) / rms.length) / 1e4;
            const result = {pluginId, between:distance(a, b), again:distance(a, again),
                levels:[a, b, again].map(level)};
            assert(result.between > 0.2 && result.between > 3 * result.again,
                'Patches did not differ beyond noise: ' + JSON.stringify(result));
            return result;
        } finally {
            include('builtin/lib/tutorial.js').stop();
            call('close');
            repl.sequence.clear();
        }
    }

    // The evolve host of evolving-sequencer loads the first file before playback and
    // the second once its slot is free, and each slot sounds in turn.
    function evolution(sounds) {
        const status = () => __replCall('evolution.status', {});
        __replCall('evolution.open', {tracks, holdMs:5000, fadeMs:300});
        try {
            const sounded = [];
            sounds.forEach(([pluginId, patch], slot) => {
                assert(waitFor(() => status()[slot].state === 0, 20000), 'Evolution slot is busy: ' + JSON.stringify(status()));
                const {id, file} = prepared(pluginId, patch);
                __replCall('evolution.load', {slot, format:'CLAP', pluginId, file});
                const loaded = waitFor(() => !status()[slot].loading && status()[slot], 60000);
                __replCall('patchJob.release', {jobId:id});
                assert(loaded && loaded.error === '' && loaded.instanceId >= 0, 'Evolution load failed: ' + JSON.stringify(loaded));
                if (slot === 0) {
                    wait(1500);
                    repl.sequence.set({track:tracks[0], tracks, steps:[[60]], bpm:120, beats:2, gate:0.5, velocity:30000});
                    uapmd.audio.setEngineEnabled(true);
                    uapmd.sequencer.startPlayback();
                }
                const heard = waitFor(() => status()[slot].state === 3 && status()[slot].gain === 1
                    && repl.test.observe().peak > 1e-4, 20000);
                assert(heard, 'Evolution slot ' + slot + ' did not sound: ' + JSON.stringify(status()));
                sounded.push(status()[slot]);
            });
            return sounded;
        } finally {
            include('builtin/lib/tutorial.js').stop();
            __replCall('evolution.close', {});
            repl.sequence.clear();
        }
    }

    function removeTracks() {
        for (const trackId of [...trackIds].reverse()) {
            const job = uapmd.sequencer.removeTrack(trackId);
            waitFor(() => uapmd.mutations.getJob(job.jobId).state !== 'pending', 30000);
        }
    }

    return {differ, evolution, removeTracks};
})();

// %% Dexed takes cartridge programs as SysEx
// Program 0 left sounding for program 1 fails here.
{
    evolve2Timbre.differ('com.digital-suburban.dexed',
        'SynprezFM/SynprezFM_14.syx/00 THE  FIX', 'SynprezFM/SynprezFM_14.syx/01 Bill Bell');
}

// %% Floe takes presets through its extension
{
    evolve2Timbre.differ('com.floe-audio.floe',
        'Celtic Harp Factory Presets/Realistic Celtic Harp.floe-preset',
        'Ocarina Factory Presets/Real Ocarina.floe-preset');
}

// %% sforzando builds its states on the state of a new instance
// The second load of the first patch reuses the slot's instance.
{
    evolve2Timbre.differ('com.Plogue Art et Technologie, Inc.sforzando',
        'Free Sounds/Programs/DSF/Clav 1.sfz', 'Free Sounds/Programs/DSF/Huge Brass.sfz');
}

// %% the evolve host of evolving-sequencer takes the same files
{
    evolve2Timbre.evolution([
        ['com.digital-suburban.dexed', 'SynprezFM/SynprezFM_14.syx/01 Bill Bell'],
        ['com.floe-audio.floe', 'Ocarina Factory Presets/Real Ocarina.floe-preset'],
    ]);
}

// %% close removes the dedicated tracks
{
    evolve2Timbre.removeTracks();
    delete globalThis.evolve2Timbre;
}
