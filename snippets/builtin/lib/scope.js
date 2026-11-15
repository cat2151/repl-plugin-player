// Resolve a track ID to its instrument, or select a specific plugin instance.
(() => {
    function targets() {
        return uapmd.sequencer.getTrackInfos().flatMap(track => track.nodes
            .filter(node => node.instanceId >= 0)
            .map(node => ({trackId:track.trackId, instanceId:node.instanceId,
                name:node.displayName || node.pluginId})));
    }
    function target() {
        const id = repl.scope.target();
        return targets().find(target => target.instanceId === id) || null;
    }

    function setTarget(options) {
        if (!options || typeof options !== 'object'
            || (options.trackId !== undefined) === (options.instanceId !== undefined))
            throw new TypeError('Specify exactly one of trackId or instanceId');
        let id = options.instanceId;
        if (options.trackId !== undefined) {
            const tracks = uapmd.sequencer.getTrackInfos();
            const index = tracks.findIndex(track => track.trackId === options.trackId);
            if (index < 0) throw new Error(repl.text('scope_target_missing', {id:options.trackId}));
            id = include('builtin/lib/plugins.js').instrumentOn(index);
            if (id < 0) throw new Error(repl.text('show_ui_00', {track:options.trackId}));
        }
        const selected = targets().find(target => target.instanceId === id);
        if (id !== -1 && !selected)
            throw new Error(repl.text('scope_target_missing', {id}));
        repl.scope.setTarget(id);
        return selected || null;
    }
    // Cycle tracks in timeline order, selecting each track's instrument.
    function cycleTrack(direction) {
        const tracks = uapmd.sequencer.getTrackInfos();
        const current = repl.scope.target();
        const index = current < 0 ? -1 : tracks.findIndex(track =>
            track.nodes.some(node => node.instanceId === current));
        const step = direction < 0 ? -1 : 1;
        const start = index < 0 ? (step > 0 ? -1 : 0) : index;
        const plugins = include('builtin/lib/plugins.js');
        for (let offset = 1; offset <= tracks.length; ++offset) {
            const next = (start + step * offset + tracks.length) % tracks.length;
            if (!tracks[next].nodes.some(node => node.instanceId >= 0)) continue;
            const id = plugins.instrumentOn(next);
            if (id >= 0) {
                repl.scope.setTarget(id);
                return target();
            }
        }
        return null;
    }
    return {targets, target, setTarget, cycleTrack};
})();
