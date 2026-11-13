// A user's cells address the instrument by track number: while a loop plays
// on a track, `use` with a patch of another plugin replaces that track's
// instrument, keeps the track's effects behind the new one, and the loop cell
// still runs afterwards. The lib's other track-addressed helpers
// (`ensureInstrument`, the tutorial's effect) work on such a track too.
// Judged by the track's plugins, the graph connections and the parameters only.
// Test-owned script: editable snippet contents are never expectations.

// %% helpers
globalThis.trackUse = (() => {
    const {assert, same, waitFor} = e2e;
    const SURGE = 'Surge XT';
    const SURGE_ID = 'org.surge-synth-team.surge-xt';
    const DEXED = 'Dexed';
    const DEXED_ID = 'com.digital-suburban.dexed';
    const EFFECT_ID = 'org.surge-synth-team.surge-xt-fx';
    const loopTrack = 0;
    const effectTrack = 1;
    // Holds throwaway instances that only provide a plugin's initial parameters.
    const spareTrack = 2;

    // Each step drains the host's queued tasks first, as a separate Run would
    // (a disposed instance leaves its track only then).
    function step(f) {
        return (...args) => {
            repl.processEvents();
            return f(...args);
        };
    }

    const playLoop = step(() => {
        const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
        playLoopParts({main:prepareLoopPart(loopTrack, 'cegb')}, 120);
    });

    // The plugins on the track as `[instanceId, pluginId]`, ordered by pluginId.
    const plugins = step(track => uapmd.sequencer.getTrackInfos()[track].nodes
        .filter(n => n.instanceId >= 0)
        .map(n => [n.instanceId, n.pluginId])
        .sort((a, b) => a[1] < b[1] ? -1 : a[1] > b[1] ? 1 : 0));

    const everyPlugin = step(() => uapmd.sequencer.getTrackInfos()
        .map((t, trackIndex) => [trackIndex, t.nodes.filter(n => n.instanceId >= 0).map(n => n.instanceId)]));

    const parameters = step(instanceId => {
        const i = uapmd.instance(instanceId);
        const values = Object.fromEntries(i.getParameters().map(p => [p.id, i.getParameterValue(p.id)]));
        assert(Object.keys(values).length > 0, `No parameters for instance ${instanceId}`);
        return values;
    });

    const changed = (before, after) =>
        Object.keys(after).filter(k => !same(before[k], after[k])).length;

    // A plugin may apply a loaded state on its audio thread, so this waits
    // while the engine runs.
    const moved = (instanceId, before) =>
        waitFor(() => changed(before, parameters(instanceId)), 3000);

    // The parameters a new instance of the plugin starts with.
    const initialParameters = step(pluginId => {
        const id = uapmd.instancing.create('CLAP', pluginId, e2e.trackId(spareTrack));
        assert(id >= 0, `Could not create ${pluginId}`);
        const values = parameters(id);
        uapmd.instance(id).dispose();
        return values;
    });

    const contains = (list, item) => list.some(p => same(p, item));

    // Runs `use`, then checks that the track's instrument is one instance of
    // `pluginId` beside `others`, and that its parameters moved away from `before`.
    function replaced(track, [name, pluginId, patchPath], others, before) {
        step(() => include('builtin/lib/patch.js').use({format:'CLAP', plugin:name, patch:patchPath, track}))();
        const now = plugins(track);
        const instruments = now.filter(p => !contains(others, p));
        assert(instruments.length === 1 && instruments[0][1] === pluginId && now.length === others.length + 1,
            `Track ${track} is not one ${pluginId} beside ${JSON.stringify(others)}: ${JSON.stringify(now)}`);
        const instance = instruments[0][0];
        const count = moved(instance, before);
        assert(count > 0, `${name} ${patchPath} on track ${track}: no parameter changed`);
        return [instance, {plugin:name, patch:patchPath, changedParameters:count}];
    }

    function expectError(options) {
        const before = everyPlugin();
        let error;
        try {
            step(() => include('builtin/lib/patch.js').use(options))();
        } catch (e) {
            error = String(e);
        }
        assert(error !== undefined, `use(${JSON.stringify(options)}) did not throw`);
        assert(error.includes('track') && same(everyPlugin(), before),
            `use(${JSON.stringify(options)}) failed wrongly: ${error}`);
        return error;
    }

    // "graphInput -> (event and audio) instrument -> effect -> graphOutput" on
    // the main buses, nothing else.
    function chained(connections, instrument, effect) {
        const end = e => `${e.type === 'plugin' ? e.instanceId : e.type}/${e.busIndex}`;
        const found = [...new Set((connections || [])
            .map(c => `${c.busType}:${end(c.source)}>${end(c.target)}`))].sort();
        const expected = [
            `event:graphInput/0>${instrument}/0`,
            `audio:graphInput/0>${instrument}/0`,
            `audio:${instrument}/0>${effect}/0`,
            `audio:${effect}/0>graphOutput/0`,
        ].sort();
        return same(found, expected);
    }

    const connections = step(track => uapmd.timeline.getTrackGraphConnections(e2e.trackId(track)));

    return {SURGE, SURGE_ID, DEXED, DEXED_ID, EFFECT_ID, loopTrack, effectTrack, spareTrack,
        step, playLoop, plugins, parameters, changed, initialParameters, replaced, expectError,
        chained, connections};
})();

// %% the loop track starts with one Surge XT
{
    const t = trackUse;
    uapmd.audio.setEngineEnabled(true);
    const start = t.plugins(t.loopTrack);
    e2e.assert(start.length === 1, `Track ${t.loopTrack} does not start with one instrument: ${JSON.stringify(start)}`);
    e2e.assert(start[0][1] === t.SURGE_ID, `Track ${t.loopTrack} does not start with ${t.SURGE}: ${JSON.stringify(start)}`);
    t.surgeInitial = t.parameters(start[0][0]);
    t.dexedInitial = t.initialParameters(t.DEXED_ID);
    ({surge:start[0][0]});
}

// %% use replaces the looping track's instrument; the loop cell keeps it
{
    const t = trackUse;
    t.playLoop();
    const [dexed, toDexed] = t.replaced(t.loopTrack,
        [t.DEXED, t.DEXED_ID, 'SynprezFM/SynprezFM_14.syx/09 Lavitar'], [], t.dexedInitial);
    t.playLoop();
    e2e.assert(e2e.same(t.plugins(t.loopTrack), [[dexed, t.DEXED_ID]]),
        "Running the loop cell again changed the track's plugins");
    t.dexed = dexed;
    t.toDexed = toDexed;
    toDexed;
}

// %% the same plugin keeps its instance and only takes the patch
{
    const t = trackUse;
    const lavitar = t.parameters(t.dexed);
    const [same, toOther] = t.replaced(t.loopTrack,
        [t.DEXED, t.DEXED_ID, 'SynprezFM/SynprezFM_14.syx/07 BellDreams'], [], lavitar);
    e2e.assert(same === t.dexed, `The same plugin got a new instance: ${t.dexed} -> ${same}`);
    t.toOther = toOther;
    toOther;
}

// %% back to Surge XT, then the tutorial's effect sits behind it
{
    const t = trackUse;
    const [surge, back] = t.replaced(t.loopTrack,
        [t.SURGE, t.SURGE_ID, 'patches_factory/Basses/Attacky.fxp'], [], t.surgeInitial);
    t.playLoop();
    t.step(() => {
        const effectTrack = t.loopTrack;
        include('builtin/lib/tutorial.js').prepareEffect(effectTrack, 'CLAP', t.EFFECT_ID);
    })();
    const now = t.plugins(t.loopTrack);
    const others = now.filter(p => p[0] !== surge);
    e2e.assert(now.length === 2 && others.length === 1 && others[0][1] === t.EFFECT_ID,
        `Track ${t.loopTrack} is not ${surge} and the effect: ${JSON.stringify(now)}`);
    const effect = others[0][0];
    const graph = t.connections(t.loopTrack);
    e2e.assert(graph.success === true && t.chained(graph.connections, surge, effect),
        `Track ${t.loopTrack} is not input -> ${surge} -> effect ${effect} -> output: ${JSON.stringify(graph)}`);
    t.surge = surge;
    ({back, tutorialEffect:graph.connections});
}

// %% ensureInstrument keeps a track's instrument and fills only an empty track, once
{
    const t = trackUse;
    const found = t.step(() => {
        const loopTrack = t.loopTrack;
        const spareTrack = t.spareTrack;
        const plugins = include('builtin/lib/plugins.js');
        return [plugins.ensureInstrument(loopTrack, {format:'CLAP', plugin:t.DEXED}),
            plugins.ensureInstrument(spareTrack, {format:'CLAP', plugin:t.DEXED}),
            plugins.ensureInstrument(spareTrack, {format:'CLAP', plugin:t.SURGE})];
    })();
    const spare = t.plugins(t.spareTrack);
    const [kept, made, again] = found;
    e2e.assert(found.length === 3 && kept === t.surge && made >= 0 && again === made
        && e2e.same(spare, [[made, t.DEXED_ID]]) && t.plugins(t.loopTrack).length === 2,
        `ensureInstrument returned ${JSON.stringify(found)}; track ${t.spareTrack} has ${JSON.stringify(spare)}`);
    ({kept, made});
}

// %% an effect after the track's instrument stays, untouched, behind the new one
{
    const t = trackUse;
    t.step(() => {
        uapmd.instancing.create('CLAP', t.SURGE_ID, e2e.trackId(t.effectTrack));
        uapmd.instancing.create('CLAP', t.EFFECT_ID, e2e.trackId(t.effectTrack));
    })();
    const before = t.plugins(t.effectTrack);
    const effect = before.filter(p => p[1] === t.EFFECT_ID);
    e2e.assert(effect.length === 1, `No single effect on track ${t.effectTrack}: ${JSON.stringify(before)}`);
    const effectId = effect[0][0];
    const effectParameters = t.parameters(effectId);
    const [instrument, dexed] = t.replaced(t.effectTrack,
        [t.DEXED, t.DEXED_ID, 'SynprezFM/SynprezFM_14.syx/09 Lavitar'], effect, t.dexedInitial);
    const graph = t.connections(t.effectTrack);
    e2e.assert(graph.success === true && t.chained(graph.connections, instrument, effectId),
        `Track ${t.effectTrack} is not input -> ${instrument} -> effect ${effectId} -> output: ${JSON.stringify(graph)}`);
    const effectChanged = t.changed(effectParameters, t.parameters(effectId));
    e2e.assert(effectChanged === 0, `The effect changed ${effectChanged} parameters`);
    ({use:dexed, connections:graph.connections});
}

// %% use without a track, or with a missing one, fails without touching instances
{
    const t = trackUse;
    const patch = 'SynprezFM/SynprezFM_14.syx/09 Lavitar';
    ({
        trackUse:true,
        dexedChanged:t.toDexed.changedParameters,
        missingTrack:t.expectError({format:'CLAP', plugin:t.DEXED, patch}),
        noTrack:t.expectError({format:'CLAP', plugin:t.DEXED, patch, track:99}),
    });
}
