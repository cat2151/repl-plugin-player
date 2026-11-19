// A track's effects: an effect preset named as the user reads it, or each one
// chosen in the player's effect browser, applied at once. Loading only defines functions.
(() => {
    const plugins = include('builtin/lib/plugins.js');
    const tracks = include('builtin/lib/tracks.js');
    const check = result => { if (!result.success) throw new Error(result.error); };
    const endOf = e => ({type:e.type, instanceId:e.instanceId, nodeId:e.nodeId || '', busIndex:e.busIndex});

    // A track whose connections were edited leaves a new plugin unconnected:
    // put it between what reached the track's output and the output.
    function joinBeforeOutput(trackIndex, effectId) {
        const trackId = tracks.idOf(trackIndex);
        const connections = plugins.connectionsOf(trackIndex);
        const isEffect = e => e.type === 'plugin' && e.instanceId === effectId;
        if (connections.some(c => isEffect(c.source) || isEffect(c.target))) return;
        const buses = new Set();
        for (const c of connections.filter(c => c.busType === 'audio' && c.target.type === 'graphOutput')) {
            check(uapmd.timeline.disconnectTrackGraphConnection(trackId, c.id));
            const source = c.source.type === 'plugin' ? plugins.pluginEnd(c.source.instanceId, c.source.busIndex) : endOf(c.source);
            check(uapmd.timeline.connectTrackGraph(trackId, 'audio', source, plugins.pluginEnd(effectId, c.target.busIndex)));
            buses.add(c.target.busIndex);
        }
        if (buses.size === 0) buses.add(0);
        for (const bus of buses)
            check(uapmd.timeline.connectTrackGraph(trackId, 'audio', plugins.pluginEnd(effectId, bus),
                {type:'graphOutput', instanceId:-1, nodeId:'', busIndex:bus}));
    }

    function create(trackIndex, entry) {
        const id = uapmd.instancing.create(entry.format, entry.pluginId, tracks.idOf(trackIndex));
        if (id < 0) throw new Error(repl.text('snippet_17'));
        return id;
    }

    // The last effect in the track's signal: the plugin other than the
    // instrument that feeds the output. -1 when there is none.
    function lastEffect(trackIndex) {
        const instrument = plugins.instrumentOn(trackIndex);
        const c = plugins.connectionsOf(trackIndex).find(c => c.busType === 'audio'
            && c.target.type === 'graphOutput' && c.source.type === 'plugin' && c.source.instanceId !== instrument);
        return c ? c.source.instanceId : -1;
    }

    // A new instance of the plugin takes the place of `previous`. Every
    // connection is laid again, because a plugin created on the track may join
    // the chain at its end by itself. Only bus 0 of `previous` carries over;
    // the new plugin may lack its other buses.
    function replace(trackIndex, previous, entry) {
        const trackId = tracks.idOf(trackIndex);
        const otherBus = e => e.type === 'plugin' && e.instanceId === previous && e.busIndex !== 0;
        const before = plugins.connectionsOf(trackIndex).filter(c => !otherBus(c.source) && !otherBus(c.target));
        const next = create(trackIndex, entry);
        for (const c of plugins.connectionsOf(trackIndex))
            check(uapmd.timeline.disconnectTrackGraphConnection(trackId, c.id));
        const end = e => e.type === 'plugin' && e.instanceId === previous ? plugins.pluginEnd(next, e.busIndex) : endOf(e);
        for (const c of before)
            check(uapmd.timeline.connectTrackGraph(trackId, c.busType, end(c.source), end(c.target)));
        uapmd.instance(previous).dispose();
        return next;
    }

    // As cat-smf-editor places an effect: an effect of the same plugin takes the
    // state; else the last effect is replaced; a track without effects gets one.
    // `stateFile(id, created)` is the file for the placed instance (null when
    // the preset went in directly), `created` telling it is new; `check()` runs
    // before an instance is created.
    function place(trackIndex, entry, stateFile, check = () => {}) {
        const nodes = tracks.info(trackIndex).nodes;
        const same = plugins.effectsOn(trackIndex).find(id => nodes.some(n => n.instanceId === id
            && n.format === entry.format && n.pluginId === entry.pluginId));
        let id = same;
        const created = id === undefined;
        if (created) {
            check();
            const last = lastEffect(trackIndex);
            if (last >= 0) {
                id = replace(trackIndex, last, entry);
            } else {
                id = create(trackIndex, entry);
                joinBeforeOutput(trackIndex, id);
            }
        }
        const file = stateFile(id, created);
        if (file !== null) plugins.loadState(id, file);
        uapmd.sequencer.setPluginBypassed(id, false);
        return id;
    }

    // options: {format, plugin, preset, value, track}, as the effect browser's cell writes them.
    function use(options) {
        const trackIndex = options.track;
        if (!Number.isInteger(trackIndex)) throw new Error(repl.text('patch_use_02'));
        tracks.info(trackIndex);
        const entry = plugins.catalogEntry(options.format, options.plugin);
        const preset = {format:entry.format, pluginId:entry.pluginId, preset:options.preset, value:options.value};
        const fileFor = (id, created) => repl.effectState({...preset, instance:id, created});
        // A preset that cannot be restored fails before any instance is created.
        return {instanceId:place(trackIndex, entry, fileFor, () => repl.patchCheck(preset)), trackIndex};
    }

    // Each preset chosen in the player's effect browser goes onto the track at
    // once and stays after the browser closes. options: {track, cell};
    // `cell:false` writes no cell for the last preset.
    function change(options) {
        const trackIndex = options.track;
        tracks.info(trackIndex);
        include('builtin/lib/browser.js').run({track:trackIndex, kind:'effect', cell:options.cell},
            event => place(trackIndex, {format:event.format, pluginId:event.pluginId}, () => event.stateFile));
    }

    return {joinBeforeOutput, use, change};
})();
