(() => {
    const tracks = include('builtin/lib/tracks.js');

    // The number of the track holding the instance.
    function trackFor(instanceId, errorKey = 'snippet_15') {
        var index = uapmd.sequencer.getTrackInfos()
            .findIndex(t => t.nodes.some(n => n.instanceId === instanceId));
        if (index < 0) throw new Error(repl.text(errorKey));
        return index;
    }

    // The catalog entry of the plugin named as the user reads it.
    function catalogEntry(format, plugin) {
        const matches = Array.from({length:uapmd.catalog.getCount()}, (_, i) => uapmd.catalog.getPluginAt(i))
            .filter(p => p.format === format && p.displayName.toLowerCase() === plugin.toLowerCase());
        // `missingPlugin` lets the player tell absence from other failures.
        if (matches.length === 0)
            throw Object.assign(new Error(repl.text('patch_use_00', {format, plugin})), {missingPlugin:true});
        if (matches.length > 1)
            throw new Error(repl.text('patch_use_01', {format, plugin, ids:matches.map(p => p.pluginId).join(', ')}));
        return matches[0];
    }

    // A track graph endpoint on the plugin; uapmd finds the plugin by nodeId.
    function pluginEnd(instanceId, busIndex = 0) {
        return {type:'plugin', instanceId, nodeId:'plugin:' + instanceId, busIndex};
    }

    // The track's connections in signal order. Only an editable (DAG) track
    // reports them, so the track becomes one; that alone keeps its sound.
    function connectionsOf(trackIndex) {
        const trackId = tracks.idOf(trackIndex);
        uapmd.timeline.ensureDagGraph(trackId);
        const graph = uapmd.timeline.getTrackGraphConnections(trackId);
        if (!graph.success) throw new Error(graph.error);
        return graph.connections;
    }

    // The instance the track's input reaches first: the plugin receiving the
    // track's events, else its audio. -1 when the track has no plugin.
    function instrumentOn(trackIndex) {
        const connections = connectionsOf(trackIndex);
        const from = bus => connections.find(c => c.busType === bus
            && c.source.type === 'graphInput' && c.target.type === 'plugin');
        const first = from('event') || from('audio');
        return first ? first.target.instanceId : -1;
    }

    // The track's plugins other than its instrument, in the track's node order.
    function effectsOn(trackIndex) {
        const track = tracks.info(trackIndex);
        const instrument = instrumentOn(trackIndex);
        const connections = connectionsOf(trackIndex);
        return track.nodes.map(n => n.instanceId).filter(id => id >= 0 && id !== instrument
            && connections.some(c=>c.source.instanceId===id || c.target.instanceId===id));
    }

    // A plugin created on a track whose connections were edited joins nothing
    // (also after reset left the track empty); it then gets the track's events
    // and feeds its output directly.
    function joinAlone(trackIndex, instanceId) {
        const joined = connectionsOf(trackIndex).some(c =>
            c.source.instanceId === instanceId || c.target.instanceId === instanceId);
        if (joined) return;
        const input = {type:'graphInput', instanceId:-1, busIndex:0};
        const output = {type:'graphOutput', instanceId:-1, busIndex:0};
        for (const [busType, source, target] of [['event', input, pluginEnd(instanceId)], ['audio', pluginEnd(instanceId), output]]) {
            const result = uapmd.timeline.connectTrackGraph(tracks.idOf(trackIndex), busType, source, target);
            if (!result.success) throw new Error(result.error);
        }
    }

    // The track's instrument; the plugin is created only when the track has none,
    // with `options.patch` (patch.js use()'s path) when given. The scope shows a created instrument when it shows nothing yet.
    function ensureInstrument(trackIndex, options) {
        const trackId = tracks.idOf(trackIndex);
        const present = instrumentOn(trackIndex);
        if (present >= 0) {
            if (repl.scope.target() === -1) repl.scope.setTarget(present);
            return present;
        }
        const entry = catalogEntry(options.format, options.plugin);
        if (options.patch !== undefined) checkPatch(entry, options.patch);
        const created = uapmd.instancing.create(entry.format, entry.pluginId, trackId);
        if (created < 0) throw new Error(repl.text('snippet_02'));
        joinAlone(trackIndex, created);
        if (options.patch !== undefined) loadPatch(created, true, entry, options.patch);
        if (repl.scope.target() === -1) repl.scope.setTarget(created);
        return created;
    }

    // uapmd reports a failed load as a result, not an exception.
    function loadState(instanceId, file) {
        const result = uapmd.instance(instanceId).loadState(file);
        if (!result.success) throw new Error(result.error);
    }

    // A patch that cannot be restored fails here, before any instance is created.
    function checkPatch(entry, patch) {
        repl.patchCheck({format:entry.format, pluginId:entry.pluginId, patch});
    }

    // Puts the catalog patch into the instance; `created` tells the instance is new.
    function loadPatch(instanceId, created, entry, patch) {
        const file = repl.patchState({format:entry.format, pluginId:entry.pluginId, patch, instance:instanceId, created});
        if (file !== null) loadState(instanceId, file);
    }

    return {trackFor, catalogEntry, pluginEnd, connectionsOf, instrumentOn, effectsOn, joinAlone, ensureInstrument, loadState, checkPatch, loadPatch};
})();
