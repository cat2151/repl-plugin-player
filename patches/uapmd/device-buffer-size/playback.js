// Set globalThis.deviceBufferProbe = {path: ..., seconds: 15} before running.
// Real device regression: measures the callback cadence, signal, and deadlines.
(() => {
    const options = globalThis.deviceBufferProbe;
    if (!options || !options.path) throw new Error('Set deviceBufferProbe.path to a project archive');
    const seconds = options.seconds || 15;
    uapmd.audio.setEngineEnabled(false);
    repl.project.load(options.path);
    if (options.frames !== undefined)
        uapmd.sequencer.reconfigureAudioDevice(-1, -1, 0, options.frames);
    if (options.workers !== undefined && !__remidy_configure_audio_workers(options.workers))
        throw new Error('Could not configure diagnostic worker count');
    const freezeStart = Date.now();
    const frozen = [];
    if (options.freeze) {
        for (const track of uapmd.sequencer.getTrackInfos()) {
            if (!uapmd.timeline.getClips(track.trackId).length) continue;
            uapmd.sequencer.setTrackFreezePolicy(track.trackId, 'on');
            let state;
            do {
                repl.processEvents();
                state = uapmd.sequencer.getTrackFreezeState(track.trackId);
                if (state.runtimeState === 'error') throw new Error(JSON.stringify(state));
                if (Date.now() - freezeStart > 240000) throw new Error('Freeze timed out: ' + JSON.stringify(state));
            } while (state.runtimeState !== 'frozen');
            if (!state.frozenAudio || !(state.frozenAudio.frames > 0))
                throw new Error('Missing frozen audio: ' + JSON.stringify(state));
            frozen.push(state);
        }
        if (!frozen.length) throw new Error('No tracks frozen');
    }
    const freezeMs = Date.now() - freezeStart;
    const passPeaks = options.loopSeconds ? Array(Math.floor(seconds / options.loopSeconds)).fill(0) : [];
    if (options.loopSeconds)
        repl.timeline.setLoop(0, Math.round(uapmd.sequencer.getSampleRate() * options.loopSeconds));
    const before = __remidy_audio_worker_diagnostics();
    __replCall('test.observe', {}); // Enable timing and drain previous records.
    let peak = 0;
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    const start = Date.now();
    let observation = start;
    let elapsed;
    let after;
    try {
        while (Date.now() - start < seconds * 1000) {
            repl.processEvents();
            if (Date.now() - observation >= 200) {
                const signal = __replCall('test.observe', {});
                if (!signal.finite) throw new Error('Non-finite audio');
                peak = Math.max(peak, signal.peak);
                const pass = Math.floor((Date.now() - start) / (1000 * options.loopSeconds));
                if (pass < passPeaks.length) passPeaks[pass] = Math.max(passPeaks[pass], signal.peak);
                observation = Date.now();
            }
        }
        elapsed = (Date.now() - start) / 1000;
        after = __remidy_audio_worker_diagnostics();
    } finally {
        uapmd.sequencer.stopPlayback();
        uapmd.audio.setEngineEnabled(false);
    }
    const blocks = after.timingCounters.realtimeBlocks - before.timingCounters.realtimeBlocks;
    const misses = after.timingCounters.deadlineMisses - before.timingCounters.deadlineMisses;
    const framesPerBlock = uapmd.sequencer.getSampleRate() * elapsed / blocks;
    const expectedFrames = options.frames || 1024;
    if (Math.abs(framesPerBlock - expectedFrames) > expectedFrames * 0.08)
        throw new Error('Unexpected device cadence: ' + framesPerBlock);
    if (after.fault || after.currentFault || misses)
        throw new Error('Audio deadline failure: ' + JSON.stringify(after));
    if (!(peak > 0)) throw new Error('No audio signal');
    if (passPeaks.some(value => value < 0.0001)) throw new Error('Silent loop pass: ' + JSON.stringify(passPeaks));
    return {blocks:blocks, inferredFramesPerBlock:framesPerBlock, deadlineMisses:misses, peak:peak,
        freezeMs:freezeMs, frozen:frozen, passPeaks:passPeaks};
})()
