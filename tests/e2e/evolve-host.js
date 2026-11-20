// evolve's host side with real plugins and test-owned pads; never reads snippet music.
// %% open on two dedicated tracks
{
    const plugins = include('builtin/lib/plugins.js');
    const patches = repl.patchCatalog({role:'chord', kind:'pad'});
    const sound = plugin => {
        const entry = plugins.catalogEntry('CLAP', plugin);
        const patch = patches.filter(p => p.format === entry.format && p.pluginId === entry.pluginId)
            .sort((a, b) => a.patch < b.patch ? -1 : a.patch > b.patch ? 1 : 0)[0];
        e2e.assert(patch, 'No pad for ' + plugin);
        // The pad as the evolve pool prepares it; kept until the session ends.
        const id = __replCall('patchJob.start', {format:entry.format, pluginId:entry.pluginId, patch:patch.patch});
        const progress = e2e.waitFor(() => {
            const value = __replCall('patchJob.get', {jobId:id});
            return value.state !== 'pending' && value;
        }, 30000);
        e2e.assert(progress && progress.state === 'completed', 'Pad was not prepared: ' + JSON.stringify(progress));
        return {format:entry.format, pluginId:entry.pluginId, file:progress.file};
    };
    const before = uapmd.sequencer.getTrackInfos().map(t => t.trackId);
    for (let i = 0; i < 2; ++i) {
        const job = uapmd.sequencer.addTrack();
        e2e.waitFor(() => uapmd.mutations.getJob(job.jobId).state !== 'pending', 30000);
        e2e.assert(uapmd.mutations.getJob(job.jobId).state !== 'failed', 'Track was not added');
    }
    const infos = uapmd.sequencer.getTrackInfos();
    const tracks = infos.map((t, index) => before.includes(t.trackId) ? -1 : index).filter(index => index >= 0);
    globalThis.evolveHost = {
        surge:sound('Surge XT'), tyrell:sound('TyrellN6'), tracks,
        trackIds:tracks.map(index => infos[index].trackId),
        // Collects outcomes until every id has settled; each must be live and error-free.
        settle(ids, ms) {
            const outcomes = [];
            e2e.waitFor(() => {
                outcomes.push(...__replCall('evolve.poll', {}));
                return ids.every(id => outcomes.some(o => o.requestId === id));
            }, ms);
            for (const id of ids) {
                const outcome = outcomes.find(o => o.requestId === id);
                e2e.assert(outcome && !outcome.stale && outcome.error === '', 'Request did not settle: ' + JSON.stringify(outcomes));
            }
            return outcomes;
        },
        submit(slot, sound) {
            const start = Date.now();
            const id = __replCall('evolve.submit', {slot, ...sound});
            e2e.assert(Date.now() - start < 100, 'Submit waited for the load');
            return id;
        },
        status: () => __replCall('evolve.status', {}),
    };
    __replCall('evolve.open', {groups:[{tracks, input:'chord'}], holdMs:5000, fadeMs:500});
    tracks
}

// %% first loads create one instance per slot
{
    const host = evolveHost;
    const ids = [host.submit(0, host.surge), host.submit(1, host.surge)];
    const outcomes = host.settle(ids, 60000);
    const status = host.status();
    e2e.assert(status.every(s => s.state === 2 && s.instanceId >= 0 && s.pluginId === host.surge.pluginId),
        'Slots are not ready: ' + JSON.stringify(status));
    e2e.assert(status[0].instance !== status[1].instance, 'Both slots share an instance');
    host.first = status;
    ({outcomes, status})
}

// %% the same kind reloads state on the retired slot's own instance
{
    const host = evolveHost;
    repl.sequence.set({track:host.tracks[0], tracks:host.tracks, steps:[[60,64,67]],
        bpm:120, beats:4, gate:1, velocity:18000});
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    // The first slot is heard while it is measured.
    const {peak} = e2e.observe(3000);
    e2e.assert(peak > 1e-4, 'evolve slot is silent: ' + peak);
    e2e.assert(e2e.waitFor(() => host.status()[0].state === 0, 30000), 'Slot 0 was not retired: ' + JSON.stringify(host.status()));
    const outcomes = host.settle([host.submit(0, host.surge)], 30000);
    const status = host.status();
    e2e.assert(status[0].instance === host.first[0].instance && status[0].instanceId === host.first[0].instanceId,
        'Same kind replaced the instance: ' + JSON.stringify({before:host.first, after:status}));
    ({peak, outcomes, status})
}

// %% another kind swaps in a new instance
{
    const host = evolveHost;
    e2e.assert(e2e.waitFor(() => host.status()[1].state === 0, 30000), 'Slot 1 was not retired: ' + JSON.stringify(host.status()));
    const outcomes = host.settle([host.submit(1, host.tyrell)], 60000);
    const status = host.status();
    e2e.assert(status[1].pluginId === host.tyrell.pluginId && status[1].instance !== host.first[1].instance,
        'Another kind did not swap: ' + JSON.stringify(status));
    e2e.assert(e2e.waitFor(() => host.status()[1].state === 3, 30000), 'TyrellN6 never sounded: ' + JSON.stringify(host.status()));
    const {peak} = e2e.observe(1000);
    e2e.assert(peak > 1e-4, 'Swapped slot is silent: ' + peak);
    ({peak, outcomes, status})
}

// %% close removes the dedicated tracks
{
    const host = evolveHost;
    include('builtin/lib/tutorial.js').stop();
    __replCall('evolve.close', {});
    repl.sequence.clear();
    for (const trackId of [...host.trackIds].reverse()) {
        const job = uapmd.sequencer.removeTrack(trackId);
        e2e.waitFor(() => uapmd.mutations.getJob(job.jobId).state !== 'pending', 30000);
    }
    const left = uapmd.sequencer.getTrackInfos().filter(t => host.trackIds.includes(t.trackId));
    e2e.assert(left.length === 0, 'Dedicated tracks remain');
    let closed = false;
    try { __replCall('evolve.status', {}); } catch (error) { closed = true; }
    e2e.assert(closed, 'evolve is still open');
    delete globalThis.evolveHost;
}
