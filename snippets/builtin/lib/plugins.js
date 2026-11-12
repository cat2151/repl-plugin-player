(() => {
    function trackFor(instanceId, errorKey = 'snippet_15') {
        var track = uapmd.sequencer.getTrackInfos()
            .find(t => t.nodes.some(n => n.instanceId === instanceId));
        if (!track) throw new Error(repl.text(errorKey));
        return track;
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
        uapmd.timeline.ensureDagGraph(trackIndex);
        const graph = uapmd.timeline.getTrackGraphConnections(trackIndex);
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
        const track = uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === trackIndex);
        if (!track) throw new Error(repl.text('patch_browse_00', {track:trackIndex}));
        const instrument = instrumentOn(trackIndex);
        return track.nodes.map(n => n.instanceId).filter(id => id >= 0 && id !== instrument);
    }

    // The track's instrument; the plugin is created only when the track has none.
    // The scope shows a created instrument when it shows nothing yet.
    function ensureInstrument(trackIndex, options) {
        if (!uapmd.sequencer.getTrackInfos().some(t => t.trackIndex === trackIndex))
            throw new Error(repl.text('patch_browse_00', {track:trackIndex}));
        const present = instrumentOn(trackIndex);
        if (present >= 0) return present;
        const entry = catalogEntry(options.format, options.plugin);
        const created = uapmd.instancing.create(entry.format, entry.pluginId, trackIndex);
        if (created < 0) throw new Error(repl.text('snippet_02'));
        if (repl.scope.target() < 0) repl.scope.setTarget(created);
        return created;
    }

    return {trackFor, catalogEntry, pluginEnd, connectionsOf, instrumentOn, effectsOn, ensureInstrument};
})();
