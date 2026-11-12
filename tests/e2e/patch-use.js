// `builtin/lib/patch.js` selects a plugin by name, restores a patch of the
// player's catalog by its display path on the named track's instrument, and
// reuses the instrument when it is that plugin already.
// Test-owned script: editable snippet contents are never expectations.

// %% helpers
globalThis.patchUse = (() => {
    const {assert, same, waitFor} = e2e;
    const patch = include('builtin/lib/patch.js');

    // Each step drains the host's queued tasks first, as a separate Run would
    // (a disposed instance leaves its track only then).
    function step(f) {
        return (...args) => {
            repl.processEvents();
            return f(...args);
        };
    }
    const use = step(options => patch.use(options));

    const instances = step(() => uapmd.sequencer.getTrackInfos().flatMap(t => t.nodes
        .filter(n => n.instanceId >= 0)
        .map(n => ({trackIndex:t.trackIndex, instanceId:n.instanceId, format:n.format,
            pluginId:n.pluginId, displayName:n.displayName}))));

    // Every parameter value of the instance, by parameter id.
    const parameters = step(instanceId => {
        const i = uapmd.instance(instanceId);
        const values = Object.fromEntries(i.getParameters().map(p => [p.id, i.getParameterValue(p.id)]));
        assert(Object.keys(values).length > 0, `No parameters for instance ${instanceId}`);
        return values;
    });

    const changed = (before, after) =>
        Object.keys(after).filter(k => !same(before[k], after[k])).length;

    // Some plugins (Surge XT) apply a loaded state on their audio thread, so
    // this waits while the engine runs.
    const moved = (instanceId, before) =>
        waitFor(() => changed(before, parameters(instanceId)), 3000);

    // Calls `use` twice; the second call must change no instance, and the
    // track must hold that plugin as its only plugin.
    function useTwice(format, plugin, patchPath, track) {
        const options = patchPath === undefined ? {format, plugin, track} : {format, plugin, patch:patchPath, track};
        const start = Date.now();
        const first = use(options);
        const firstMs = Date.now() - start;
        const afterFirst = instances();
        const second = use(options);
        const afterSecond = instances();
        assert(same(first, second) && same(afterFirst, afterSecond),
            `Second use changed the instance: ${JSON.stringify([first, second, afterFirst, afterSecond])}`);
        const node = afterSecond.find(n => n.instanceId === first.instanceId);
        assert(node, `Instance ${first.instanceId} is not on any track`);
        const onTrack = afterSecond.filter(n => n.trackIndex === track).length;
        assert(node.format === format && node.displayName === plugin && node.trackIndex === track
            && first.trackIndex === track && onTrack === 1,
            `Track ${track} is not one ${format} ${plugin}: ${JSON.stringify([first, afterSecond])}`);
        return {result:first, node, firstMs};
    }

    // `use` with a patch on the instance `use` without a patch returned; the
    // parameters must move.
    function restores(plugin, patchPath, track) {
        const plain = useTwice('CLAP', plugin, undefined, track);
        const instanceId = plain.result.instanceId;
        const before = parameters(instanceId);
        const patched = useTwice('CLAP', plugin, patchPath, track);
        assert(patched.result.instanceId === instanceId,
            `use with a patch moved the instance: ${JSON.stringify(patched)}`);
        const changedParameters = moved(instanceId, before);
        assert(changedParameters > 0, `${plugin} ${patchPath}: no parameter changed`);
        return {patch:patchPath, use:patched, changedParameters, parameterCount:Object.keys(before).length};
    }

    function expectError(options, needle) {
        const before = instances();
        let error;
        try {
            use(options);
        } catch (e) {
            error = String(e);
        }
        assert(error !== undefined, `use(${JSON.stringify(options)}) did not throw`);
        assert(error.includes(needle) && same(instances(), before),
            `use(${JSON.stringify(options)}) failed wrongly: ${error}`);
        return error;
    }

    return {instances, useTwice, restores, expectError};
})();

// %% the track's Surge XT is reused, not duplicated
{
    const track = 0;
    uapmd.audio.setEngineEnabled(true);
    const start = patchUse.instances();
    const surge = patchUse.restores('Surge XT', 'patches_factory/Basses/Attacky.fxp', track);
    const reused = surge.use.result.instanceId;
    e2e.assert(surge.use.node.pluginId === 'org.surge-synth-team.surge-xt'
        && start.some(n => n.instanceId === reused),
        `CLAP use did not reuse the track's instrument: ${JSON.stringify([surge, start])}`);
    surge;
}

// %% Dexed has no uapmd presets; its patch is a voice inside a cartridge
{
    const track = 0;
    const dexed = patchUse.restores('Dexed', 'SynprezFM/SynprezFM_14.syx/09 Lavitar', track);
    e2e.assert(dexed.use.node.pluginId === 'com.digital-suburban.dexed',
        `Dexed did not replace the instrument: ${JSON.stringify(dexed)}`);
    dexed;
}

// %% a missing patch or plugin fails without touching instances
{
    const track = 0;
    ({
        missingPatch: patchUse.expectError({format:'CLAP', plugin:'Surge XT', patch:'no such patch', track}, 'no such patch'),
        missingPlugin: patchUse.expectError({format:'CLAP', plugin:'no such plugin', patch:'x', track}, 'no such plugin'),
    });
}

// %% a track without a plugin gets one; the patch is optional
{
    const emptyTrack = 1;
    const hasVst3 = Array.from({length:uapmd.catalog.getCount()}, (_, i) => uapmd.catalog.getPluginAt(i))
        .some(p => p.format === 'VST3' && p.displayName === 'Surge XT');
    const format = hasVst3 ? 'VST3' : 'CLAP';
    const base = patchUse.instances().length;
    const created = patchUse.useTwice(format, 'Surge XT', undefined, emptyTrack);
    e2e.assert(patchUse.instances().length === base + 1,
        `use on an empty track did not create exactly one instance: ${JSON.stringify(created)}`);
    ({created, createdFormat:format});
}
