// Test-owned material; builtin snippets are smoke-tested only.
// %% prepare
{
    const melodyTrack = 0;
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'cde', 120);
    globalThis.freezeTrackId = include('builtin/lib/tracks.js').idOf(melodyTrack);
    globalThis.freezeMinimumFrames = 0;
}

// %% on
uapmd.sequencer.setTrackFreezePolicy(freezeTrackId, 'on');

// %% start transport
uapmd.sequencer.startPlayback();

// %% waiting for stop
(() => {
    const state = uapmd.sequencer.getTrackFreezeState(freezeTrackId);
    if (state.policy !== 'on' || state.runtimeState !== 'live' || state.frozenAudio != null)
        throw new Error('Expected deferred freeze during playback: ' + JSON.stringify(state));
    const text = include('builtin/lib/freeze.js').describe(freezeTrackId);
    if (!text.includes('Waiting for playback to stop') || !text.includes('auto freeze on')
        || text.includes('null') || text.includes('0%'))
        throw new Error('Unclear deferred freeze: ' + text);
    return text;
})();

// %% stop transport
uapmd.sequencer.stopPlayback();

// %% automatic freeze playback
{
    globalThis.freezeMinimumFrames = 0;
    repl.timeline.clearLoop();
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    uapmd.sequencer.setTrackFreezePolicy(freezeTrackId, 'on');
}

// %% display states
(() => {
    const sequencer = uapmd.sequencer;
    const original = sequencer.getTrackFreezeState;
    const describe = include('builtin/lib/freeze.js').describe;
    const cases = [
        [{policy:'off', runtimeState:'live'}, 'freeze disabled'],
        [{policy:'on', runtimeState:'live'}, 'Waiting to render'],
        [{policy:'on', runtimeState:'rendering', totalFrames:0}, 'Rendering frozen audio'],
        [{policy:'on', runtimeState:'rendering', totalFrames:48000,
            renderProgress:0.5, renderedSeconds:0.5, totalSeconds:1}, '50% / 0.50 / 1.00 s'],
        [{policy:'on', runtimeState:'frozen', frozenAudio:{frames:48000, channels:2}},
            '48000 frames / 2 ch'],
        [{policy:'on', runtimeState:'error', error:'test render failure'}, 'test render failure']
    ];
    try {
        for (const [state, expected] of cases) {
            sequencer.getTrackFreezeState = () => state;
            const text = describe(freezeTrackId);
            if (!text.includes(expected) || text.includes('null') || text.includes('undefined'))
                throw new Error('Unclear freeze display: ' + text);
        }
    } finally {
        sequencer.getTrackFreezeState = original;
    }
    return true;
})();

// %% ready
(() => {
    const state = uapmd.sequencer.getTrackFreezeState(freezeTrackId);
    if (state.runtimeState === 'error') throw new Error(JSON.stringify(state));
    if (state.runtimeState !== 'frozen' || !state.frozenAudio ||
        state.frozenAudio.frames <= (globalThis.freezeMinimumFrames || 0)) {
        globalThis.freezeStableSince = Date.now();
        return false;
    }
    if (!globalThis.freezeStableSince) globalThis.freezeStableSince = Date.now();
    return Date.now() - globalThis.freezeStableSince >= 100;
})();

// %% state
uapmd.sequencer.getTrackFreezeState(freezeTrackId);

// %% edit
{
    const melodyTrack = 0;
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'l4 cdefga', 120);
}

// %% off
uapmd.sequencer.setTrackFreezePolicy(freezeTrackId, 'off');

// %% loop playback
(() => {
    const rate = uapmd.sequencer.getSampleRate();
    const peaks = [0, 0, 0, 0, 0];
    const range = globalThis.freezeLoopRange || [0, rate];
    repl.timeline.setLoop(range[0], range[1]);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    const start = Date.now();
    let last = start;
    try {
        while (Date.now() - start < 5000) {
            repl.processEvents();
            if (Date.now() - last < 50) continue;
            last = Date.now();
            const signal = repl.test.observe();
            if (!signal.finite) throw new Error('Non-finite frozen loop audio');
            const pass = Math.floor((last - start) / 1000);
            if (pass < peaks.length) peaks[pass] = Math.max(peaks[pass], signal.peak);
        }
    } finally {
        uapmd.sequencer.stopPlayback();
        uapmd.audio.setEngineEnabled(false);
        repl.timeline.clearLoop();
    }
    if (peaks.some(peak => peak < 0.0001)) throw new Error('Frozen loop went silent: ' + JSON.stringify(peaks));
    return peaks;
})();

// %% nonzero loop range
{
    const rate = uapmd.sequencer.getSampleRate();
    const start = Math.round(rate / 8);
    globalThis.freezeLoopRange = [start, start + Math.round(rate * 0.6172835)];
}

// %% snippet smoke
include('builtin/freeze-track.js');
include('builtin/show-track-freeze.js');
include('builtin/unfreeze-track.js');

// %% save
repl.project.saveArchive(freezeProjectPath);

// %% load
repl.project.load(freezeProjectPath);
globalThis.freezeTrackId = uapmd.sequencer.getTrackInfos()[0].trackId;

// %% load while rendering
{
    const melodyTrack = 0;
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'l4 cdefgab<c', 120);
    const state = uapmd.sequencer.getTrackFreezeState(freezeTrackId);
    if (state.runtimeState !== 'rendering')
        throw new Error('Expected active freeze before project load: ' + JSON.stringify(state));
    repl.project.load(freezeProjectPath);
    globalThis.freezeTrackId = uapmd.sequencer.getTrackInfos()[0].trackId;
}

// %% invalid load while rendering
{
    const melodyTrack = 0;
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'l4 cdefgab<c', 120);
    if (uapmd.sequencer.getTrackFreezeState(freezeTrackId).runtimeState !== 'rendering')
        throw new Error('Expected active freeze before invalid project load');
    for (const path of [freezeProjectPath + '.missing', freezeInvalidProjectPath]) {
        let rejected = false;
        try {
            repl.project.load(path);
        } catch (error) {
            if (!String(error).includes('project.load:')) throw error;
            rejected = true;
        }
        if (!rejected) throw new Error('Invalid project was accepted');
    }
    const state = uapmd.sequencer.getTrackFreezeState(freezeTrackId);
    if (state.policy !== 'on')
        throw new Error('Failed load changed outgoing freeze: ' + JSON.stringify(state));
}
