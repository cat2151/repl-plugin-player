// A timeline clip plays its model note; Run keeps globals and clips, an explicit
// reset replaces the clip, an error keeps the edits made before it, and Stop
// keeps the clips with the transport stopped.
// Test-owned script: editable snippet contents are never expectations.

// %% a global the later cells read
globalThis.retainedValue = 42;

// %% one clip
e2e.timelineNote();

// %% the model note and its signal
(() => {
    const {assert, clips, notes, observe} = e2e;
    const read = clips(0);
    assert(read.length === 1, `Expected one clip: ${JSON.stringify(read)}`);
    const {tickResolution: tpqn, events} = read[0].midi;
    const model = notes(events);
    assert(model.length === 1 && model[0].pitch === 60 && model[0].start === tpqn
        && model[0].end === 3 * tpqn, `Wrong model note: ${JSON.stringify(read[0].midi)}`);
    const before = observe(300).peak;
    const held = observe(900).peak;
    observe(900);
    const release = observe(400).peak;
    assert(before <= 1e-6 && held > 1e-3 && release <= 1e-5,
        `Timeline signal failed: before=${before} held=${held} release=${release}`);
    return {before, held, release};
})();

// %% an explicit reset keeps one clip
e2e.timelineNote();
e2e.assert(e2e.clips(0).length === 1, 'Explicit reset did not keep one clip');

// %% without the reset the clips accumulate
e2e.timelineNote({reset: false});
globalThis.accumulated = JSON.stringify(e2e.clips(0));
e2e.assert(e2e.clips(0).length === 2, 'Without reset the clips did not accumulate');

// %% reads do not change the model
for (let read = 0; read < 3; read++)
    e2e.assert(JSON.stringify(e2e.clips(0)) === accumulated, 'Read changed model content');
e2e.assert(retainedValue === 42, 'Run unexpectedly reset globals');

// %% error - after a model edit
uapmd.timeline.createEmptyMidiClip(0, 0);
throw new Error('after model edit');

// %% the edit before the failure stays
e2e.assert(e2e.clips(0).length === 3, 'Partial model edits were hidden or rolled back after an error');

// %% stop - keeps the clips
e2e.assert(uapmd.timeline.getState().isPlaying === false && e2e.clips(0).length === 3,
    'Stop discarded clips or left transport playing');
({resetClipCount: 1, accumulatedClipCount: 2, afterErrorClipCount: 3});
