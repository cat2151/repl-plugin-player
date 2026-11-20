// The user's evolve flow: try-evolving-sequencer plays and switches sounds, and
// try-end-evolve leaves nothing behind. Snippet music is not asserted; only the
// candidate plugins are fixed, and only to force a switch kind.
// %% helpers
{
    const silentPeak = 1e-4;
    const runSnippet = name => {
        const source = __replCall('snippet.source', {name:'builtin/' + name + '.js'});
        for (const cell of source.split(/^\/\/ %%.*$/m).filter(c => c.trim())) (0, eval)(cell);
    };
    const run = {
        silentPeak, runSnippet,
        switches:[], seen:new Set(), active:[undefined, undefined], engaged:[undefined, undefined], states:[0, 0],
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
                const status = __replCall('evolve.status', {});
                status.forEach((slot, index) => {
                    if (slot.instanceId >= 0) run.seen.add(slot.instanceId);
                    // A slot is engaged: Ready (or a state the poll missed) becomes Active.
                    if (slot.state === 3 && run.states[index] !== 3 && run.states[index] !== 4)
                        run.engaged[index] = {slot:index, pluginId:slot.pluginId, instance:slot.instance, at:now};
                    // It switches only once fully heard; a slot retired for a deadline miss never is.
                    const next = run.engaged[index];
                    if (next && slot.state === 3 && slot.gain === 1) {
                        const previous = run.active[index];
                        if (previous) run.switches.push({...next, sameKind:previous.pluginId === next.pluginId,
                            sameInstance:previous.instance === next.instance, from:previous.pluginId});
                        run.active[index] = next;
                        run.engaged[index] = undefined;
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
            status:__replCall('evolve.status', {}),
            buffers:globalThis.evolvePool && globalThis.evolvePool.buffers.map(b => ({slot:b.slot, status:b.status,
                sound:b.sound && b.sound.patch, request:b.request, remaining:b.remaining}))})
    };
    globalThis.evolveRun = run;
}

// %% try-evolving-sequencer keeps sounding across two switches
{
    const run = evolveRun;
    run.instancesBefore = repl.test.instances();
    run.runSnippet('try-evolving-sequencer');
    run.trackIds = [...globalThis.evolveMode.trackIds];
    e2e.assert(run.watch(() => run.switches.length >= 2, 60000), 'try-evolving-sequencer did not switch twice: ' + JSON.stringify(run.summary()));
    e2e.assert(run.startedAt !== undefined, 'try-evolving-sequencer never sounded');
    const held = repl.test.instances();
    e2e.assert([...run.seen].every(id => held.includes(id)), 'The host does not list the evolve instances');
    run.summary()
}

// %% the same kind reloads state on its own instance
{
    const run = evolveRun;
    const from = run.switches.length;
    const surge = run.only('Surge XT');
    try { e2e.assert(run.watch(() => run.count(from, surge, true) >= 1, 60000), 'No same-kind switch: ' + JSON.stringify(run.summary())); }
    finally { run.restoreCatalog(); }
    run.summary()
}

// %% another kind swaps the instance
{
    const run = evolveRun;
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
    const run = evolveRun;
    run.runSnippet('try-end-evolve');
    e2e.assert(!globalThis.evolveMode, 'evolve mode is still on');
    const left = uapmd.sequencer.getTrackInfos().filter(t => run.trackIds.includes(t.trackId));
    e2e.assert(left.length === 0, 'Dedicated tracks remain: ' + JSON.stringify(left));
    let closed = false;
    try { __replCall('evolve.status', {}); } catch (error) { closed = true; }
    e2e.assert(closed, 'evolve is still open');
    const remaining = () => repl.test.instances().filter(id => run.seen.has(id));
    e2e.assert(e2e.waitFor(() => remaining().length === 0, 5000), 'evolve instances remain: ' + JSON.stringify(remaining()));
    delete globalThis.evolveRun;
    ({seen:[...run.seen], before:run.instancesBefore, after:repl.test.instances()})
}
