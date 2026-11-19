// Patch cells on a track's plugin, as the user runs them: a new instance takes
// a patch, the same instance takes another, then the first again. Each cell
// returns the wall-clock window of `patch.use` and of the settling after it,
// for `scripts/e2e.py` to count the frame monitor's stalls per cell, and how
// many parameter values the patch changed.
// Test-owned script: editable snippet contents are never expectations.

// %% helpers
globalThis.patchRescan = (() => {
    const {assert} = e2e;
    // `e2e.wait` beats the frame monitor once at its end; short waits keep a
    // quiet stretch free of stalls, so a stall is the work it overlaps.
    const wait = ms => {
        const end = Date.now() + ms;
        while (Date.now() < end) e2e.wait(10);
    };
    const tracks = {};
    const last = {};

    // The instance's values, and how long the list and the values took to read.
    const values = instanceId => {
        const instance = uapmd.instance(instanceId);
        const start = Date.now();
        const list = instance.getParameters();
        const listed = Date.now();
        const read = Object.fromEntries(list.map(p => [p.id, instance.getParameterValue(p.id)]));
        return {read, readMs:{list:listed - start, values:Date.now() - listed}};
    };

    // A new track per plugin, so the first patch creates the instance.
    function trackFor(pluginId) {
        if (tracks[pluginId] === undefined) {
            const job = uapmd.sequencer.addTrack();
            assert(job.state === 'completed', `addTrack did not add a track: ${JSON.stringify(job)}`);
            tracks[pluginId] = uapmd.sequencer.getTrackInfos().length - 1;
        }
        return tracks[pluginId];
    }

    // `settleMs` keeps the control loop pumping after `use` returns, so work the
    // plugin asks for on the main thread afterwards falls inside the cell's window.
    function use(pluginId, patch, settleMs = 1500) {
        uapmd.audio.setEngineEnabled(true);
        const track = trackFor(pluginId);
        repl.processEvents();
        wait(300);
        const before = last[pluginId];
        const startMs = Date.now();
        const {instanceId} = include('builtin/lib/patch.js').use({track, format:'CLAP', pluginId, patch});
        const usedMs = Date.now();
        wait(settleMs);
        const endMs = Date.now();
        const {read:after, readMs} = values(instanceId);
        const created = !before || before.instanceId !== instanceId;
        const changed = created ? null
            : Object.keys(after).filter(id => after[id] !== before.values[id]).length;
        last[pluginId] = {instanceId, values:after};
        return {pluginId, patch, instanceId, created, startMs, usedMs, endMs,
            useMs:usedMs - startMs, parameters:Object.keys(after).length, changed, readMs};
    }

    // How many of the instance's last read values differ from `earlier`.
    function differingFrom(pluginId, earlier) {
        const now = last[pluginId].values;
        return Object.keys(now).filter(id => now[id] !== earlier[id]).length;
    }

    const snapshot = pluginId => ({...last[pluginId].values});

    return {use, differingFrom, snapshot};
})();

// %% Six Sines first patch on a new instance
{
    const sixSines = 'org.baconpaul.six-sines';
    const result = patchRescan.use(sixSines, 'Bass/Bass 1.sxsnp');
    globalThis.patchRescanFirst = {[sixSines]:patchRescan.snapshot(sixSines)};
    result;
}

// %% Six Sines second patch on the same instance
{
    const result = patchRescan.use('org.baconpaul.six-sines', 'Leads/Airlock.sxsnp');
    e2e.assert(!result.created && result.changed > 0, `The second patch changed no value: ${JSON.stringify(result)}`);
    result;
}

// %% Six Sines first patch again
{
    const sixSines = 'org.baconpaul.six-sines';
    const result = patchRescan.use(sixSines, 'Bass/Bass 1.sxsnp');
    e2e.assert(!result.created && result.changed > 0, `The first patch again changed no value: ${JSON.stringify(result)}`);
    result.differsFromFirst = patchRescan.differingFrom(sixSines, patchRescanFirst[sixSines]);
    result;
}

// %% Vaporizer2 first patch on a new instance
{
    const plugin = include('builtin/lib/plugins.js').catalogEntry('CLAP', 'Vaporizer2');
    const patches = [...new Set(repl.patchCatalog({role:'chord', kind:'pad'})
        .filter(p => p.format === plugin.format && p.pluginId === plugin.pluginId).map(p => p.patch))].sort();
    e2e.assert(patches.length >= 2, `Vaporizer2 has fewer than two pads: ${JSON.stringify(patches)}`);
    globalThis.patchRescanVaporizer = {pluginId:plugin.pluginId, patches:patches.slice(0, 2)};
    const result = patchRescan.use(plugin.pluginId, patches[0]);
    patchRescanFirst[plugin.pluginId] = patchRescan.snapshot(plugin.pluginId);
    result;
}

// %% Vaporizer2 second patch on the same instance
{
    const {pluginId, patches} = patchRescanVaporizer;
    const result = patchRescan.use(pluginId, patches[1]);
    e2e.assert(!result.created && result.changed > 0, `The second patch changed no value: ${JSON.stringify(result)}`);
    result;
}

// %% Vaporizer2 first patch again
{
    const {pluginId, patches} = patchRescanVaporizer;
    const result = patchRescan.use(pluginId, patches[0]);
    e2e.assert(!result.created && result.changed > 0, `The first patch again changed no value: ${JSON.stringify(result)}`);
    result.differsFromFirst = patchRescan.differingFrom(pluginId, patchRescanFirst[pluginId]);
    result;
}
