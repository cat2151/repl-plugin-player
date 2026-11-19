// The user's evolve2 flow: try-evolve2 plays and switches sounds, try-end-evolve leaves
// nothing behind, and try-evolving-sequencer opens afterwards. Snippet music is not
// asserted; only the candidate plugins are fixed, and only to force a switch kind.
// %% helpers
{
    const silentPeak = 1e-4;
    const runSnippet = name => {
        const source = __replCall('snippet.source', {name:'builtin/' + name + '.js'});
        for (const cell of source.split(/^\/\/ %%.*$/m).filter(c => c.trim())) (0, eval)(cell);
    };
    const run = {
        silentPeak, runSnippet,
        switches:[], seen:new Set(), active:[undefined, undefined], states:[0, 0],
        startedAt:undefined, longestSilenceMs:0, silentSince:undefined, samples:0,
        // Samples the master output and both slots until `done()` or `ms` passes.
        watch(done, ms) {
            const end = Date.now() + ms;
            while (!done() && Date.now() < end) {
                const now = Date.now();
                const {peak, finite} = repl.test.observe();
                e2e.assert(finite, 'Non-finite master output');
                ++run.samples;
                if (peak > silentPeak) {
                    if (run.startedAt === undefined) run.startedAt = now;
                    if (run.silentSince !== undefined)
                        run.longestSilenceMs = Math.max(run.longestSilenceMs, now - run.silentSince);
                    run.silentSince = undefined;
                } else if (run.startedAt !== undefined && run.silentSince === undefined) run.silentSince = now;
                const status = __replCall('evolve2.status', {});
                status.forEach((slot, index) => {
                    if (slot.instanceId >= 0) run.seen.add(slot.instanceId);
                    // A slot starts sounding: Ready (or a state the poll missed) becomes Active.
                    if (slot.state === 3 && run.states[index] !== 3 && run.states[index] !== 4) {
                        const previous = run.active[index];
                        const next = {slot:index, pluginId:slot.pluginId, instance:slot.instance, at:now};
                        if (previous) run.switches.push({...next, sameKind:previous.pluginId === slot.pluginId,
                            sameInstance:previous.instance === slot.instance, from:previous.pluginId});
                        run.active[index] = next;
                    }
                    run.states[index] = slot.state;
                });
                e2e.wait(25);
            }
            // Before the first sound the slots are still loading; after it nothing may drop out.
            e2e.assert(run.longestSilenceMs < 300, 'Master output went silent: ' + JSON.stringify(run.summary()));
            return done();
        },
        // Every later request draws only `plugin`'s patches; the snippet stays as written.
        only(plugin) {
            const entry = include('builtin/lib/plugins.js').catalogEntry('CLAP', plugin);
            const catalog = run.catalog || (run.catalog = repl.patchCatalog);
            repl.patchCatalog = options => catalog(options).filter(p => p.format === entry.format && p.pluginId === entry.pluginId);
            return entry.pluginId;
        },
        restoreCatalog() { if (run.catalog) repl.patchCatalog = run.catalog; delete run.catalog; },
        // Switches since switch number `from` into `pluginId`, of the same kind or another.
        count: (from, pluginId, same) => run.switches.slice(from)
            .filter(s => s.pluginId === pluginId && s.sameKind === same).length,
        summary: () => ({switches:run.switches.map(s => ({slot:s.slot, from:s.from, to:s.pluginId, sameInstance:s.sameInstance})),
            longestSilenceMs:run.longestSilenceMs, samples:run.samples, sounded:run.startedAt !== undefined,
            status:__replCall('evolve2.status', {}),
            buffers:globalThis.evolutionPool && globalThis.evolutionPool.buffers.map(b => ({slot:b.slot, status:b.status,
                sound:b.sound && b.sound.patch, request:b.request, remaining:b.remaining}))})
    };
    globalThis.evolve2Run = run;
}

// %% try-evolve2 keeps sounding across two switches
{
    const run = evolve2Run;
    run.instancesBefore = repl.test.instances();
    run.runSnippet('try-evolve2');
    run.trackIds = [...globalThis.evolutionMode.trackIds];
    e2e.assert(run.watch(() => run.switches.length >= 2, 60000), 'try-evolve2 did not switch twice: ' + JSON.stringify(run.summary()));
    e2e.assert(run.startedAt !== undefined, 'try-evolve2 never sounded');
    const held = repl.test.instances();
    e2e.assert([...run.seen].every(id => held.includes(id)), 'The host does not list the evolve2 instances');
    run.summary()
}

// %% the same kind reloads state on its own instance
{
    const run = evolve2Run;
    const from = run.switches.length;
    const surge = run.only('Surge XT');
    try { e2e.assert(run.watch(() => run.count(from, surge, true) >= 1, 60000), 'No same-kind switch: ' + JSON.stringify(run.summary())); }
    finally { run.restoreCatalog(); }
    run.summary()
}

// %% another kind swaps the instance
{
    const run = evolve2Run;
    const from = run.switches.length;
    const tyrell = run.only('TyrellN6');
    try { e2e.assert(run.watch(() => run.count(from, tyrell, false) >= 1, 60000), 'No kind change: ' + JSON.stringify(run.summary())); }
    finally { run.restoreCatalog(); }
    for (const s of run.switches) e2e.assert(s.sameKind === s.sameInstance,
        'A same-kind switch replaced its instance, or another kind kept it: ' + JSON.stringify(s));
    run.summary()
}

// %% try-end-evolve leaves no dedicated track or instance
{
    const run = evolve2Run;
    run.runSnippet('try-end-evolve');
    e2e.assert(!globalThis.evolutionMode, 'Evolve mode is still on');
    const left = uapmd.sequencer.getTrackInfos().filter(t => run.trackIds.includes(t.trackId));
    e2e.assert(left.length === 0, 'Dedicated tracks remain: ' + JSON.stringify(left));
    let closed = false;
    try { __replCall('evolve2.status', {}); } catch (error) { closed = true; }
    e2e.assert(closed, 'evolve2 is still open');
    const remaining = () => repl.test.instances().filter(id => run.seen.has(id));
    e2e.assert(e2e.waitFor(() => remaining().length === 0, 5000), 'evolve2 instances remain: ' + JSON.stringify(remaining()));
    ({seen:[...run.seen], before:run.instancesBefore, after:repl.test.instances()})
}

// %% try-evolving-sequencer opens after evolve2
{
    const run = evolve2Run;
    run.runSnippet('try-evolving-sequencer');
    const trackIds = [...globalThis.evolutionMode.trackIds];
    const sounding = e2e.waitFor(() => __replCall('evolution.status', {}).some(s => s.state === 3)
        && repl.test.observe().peak > run.silentPeak, 30000);
    e2e.assert(sounding, 'try-evolving-sequencer did not sound: ' + JSON.stringify(__replCall('evolution.status', {})));
    run.runSnippet('try-end-evolve');
    e2e.assert(!uapmd.sequencer.getTrackInfos().some(t => trackIds.includes(t.trackId)), 'Evolve tracks remain');
    delete globalThis.evolve2Run;
}
