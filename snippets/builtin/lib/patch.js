// Select a plugin by the name the user reads and a patch by its path in the player's catalog,
// or audition patches from the player's browser. Loading only defines functions.
(() => {
    // uapmd reports a failed load as a result, not an exception.
    function loadState(instanceId, file) {
        const result = uapmd.instance(instanceId).loadState(file);
        if (!result.success) throw new Error(result.error);
    }

    const plugins = include('builtin/lib/plugins.js');
    const pluginEnd = instanceId => plugins.pluginEnd(instanceId);

    // Wires `next` where `previous` was: the track's input reaches it, and it
    // feeds what `previous` fed (the track's effects, or the output). Every
    // connection is laid again, because a plugin created on the track joins
    // the chain behind its effects. Other connections stay as `before` had them.
    function wireInstead(trackIndex, previous, next, before) {
        for (const c of plugins.connectionsOf(trackIndex)) {
            const result = uapmd.timeline.disconnectTrackGraphConnection(trackIndex, c.id);
            if (!result.success) throw new Error(result.error);
        }
        const wanted = new Map();
        const end = e => e.type === 'plugin' && e.instanceId === previous ? pluginEnd(next)
            : {type:e.type, instanceId:e.instanceId, nodeId:e.nodeId || '', busIndex:e.busIndex};
        const add = (busType, source, target) =>
            wanted.set(JSON.stringify([busType, source, target]), [busType, source, target]);
        add('event', end({type:'graphInput', instanceId:-1, busIndex:0}), pluginEnd(next));
        add('audio', end({type:'graphInput', instanceId:-1, busIndex:0}), pluginEnd(next));
        // Only the main output of `previous` carries over; `next` may lack its other buses.
        const carried = before.filter(c => !(c.source.instanceId === previous && c.source.busIndex !== 0));
        for (const c of carried) add(c.busType, end(c.source), end(c.target));
        if (!before.some(c => c.busType === 'audio' && c.source.instanceId === previous))
            add('audio', pluginEnd(next), end({type:'graphOutput', instanceId:-1, busIndex:0}));
        for (const [busType, source, target] of wanted.values()) {
            const result = uapmd.timeline.connectTrackGraph(trackIndex, busType, source, target);
            if (!result.success) throw new Error(result.error);
        }
    }

    // The track's instrument becomes the plugin: the instrument itself when it
    // is that plugin already, else a new instance in its place, also on the scope.
    function instrumentFor(trackIndex, entry) {
        const before = plugins.connectionsOf(trackIndex);
        const previous = plugins.instrumentOn(trackIndex);
        const node = uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === trackIndex)
            .nodes.find(n => n.instanceId === previous);
        if (node && node.format === entry.format && node.pluginId === entry.pluginId) return previous;
        const next = uapmd.instancing.create(entry.format, entry.pluginId, trackIndex);
        if (next < 0) throw new Error(repl.text('snippet_02'));
        wireInstead(trackIndex, previous, next, before);
        if (previous >= 0 && repl.scope.target() === previous) repl.scope.setTarget(next);
        if (previous >= 0) uapmd.instance(previous).dispose();
        return next;
    }

    function use(options) {
        const trackIndex = options.track;
        if (!Number.isInteger(trackIndex)) throw new Error(repl.text('patch_use_02'));
        if (!uapmd.sequencer.getTrackInfos().some(t => t.trackIndex === trackIndex))
            throw new Error(repl.text('patch_browse_00', {track:trackIndex}));
        const entry = plugins.catalogEntry(options.format, options.plugin);
        // A patch that cannot be restored fails before any instance is created.
        var file = options.patch === undefined ? undefined
            : repl.patchState({format:entry.format, pluginId:entry.pluginId, patch:options.patch});
        const instanceId = instrumentFor(trackIndex, entry);
        if (file !== undefined) loadState(instanceId, file);
        uapmd.sequencer.setPluginBypassed(instanceId, false);
        return {instanceId, trackIndex};
    }

    // A plugin created on a track whose connections were edited joins nothing;
    // the audition then gets the track's events and feeds its output directly.
    function joinAlone(trackIndex, instanceId) {
        const joined = plugins.connectionsOf(trackIndex).some(c =>
            c.source.instanceId === instanceId || c.target.instanceId === instanceId);
        if (joined) return;
        const input = {type:'graphInput', instanceId:-1, busIndex:0};
        const output = {type:'graphOutput', instanceId:-1, busIndex:0};
        for (const [busType, source, target] of [['event', input, pluginEnd(instanceId)], ['audio', pluginEnd(instanceId), output]]) {
            const result = uapmd.timeline.connectTrackGraph(trackIndex, busType, source, target);
            if (!result.success) throw new Error(result.error);
        }
    }

    // The sound being auditioned: one instance, added to the browsed track.
    function audition(trackIndex, preview, event) {
        if (preview.instanceId === undefined || preview.format !== event.format || preview.pluginId !== event.pluginId) {
            const id = uapmd.instancing.create(event.format, event.pluginId, trackIndex);
            if (id < 0) throw new Error(repl.text('snippet_02'));
            joinAlone(trackIndex, id);
            if (preview.instanceId !== undefined) uapmd.instance(preview.instanceId).dispose();
            Object.assign(preview, {instanceId:id, format:event.format, pluginId:event.pluginId});
        }
        loadState(preview.instanceId, event.stateFile);
        uapmd.sequencer.setPluginBypassed(preview.instanceId, false);
    }

    // Auditions each patch chosen in the player's browser on the track while it
    // keeps playing; closing the browser brings back the track's own sound.
    function browse(options) {
        var track = uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === options.track);
        if (!track) throw new Error(repl.text('patch_browse_00', {track:options.track}));
        // The track's instrument stays untouched, only bypassed.
        const instrument = plugins.instrumentOn(track.trackIndex);
        var bypassed = instrument >= 0 && uapmd.sequencer.isPluginBypassed(instrument);
        var preview = {};
        repl.patchBrowser.open({track:track.trackIndex});
        try {
            for (;;) {
                repl.processEvents();
                for (var event of repl.patchBrowser.poll()) {
                    if (event.kind === 'closed') {
                        if (event.error) throw new Error(event.error);
                        return;
                    }
                    var error;
                    try {
                        if (instrument >= 0) uapmd.sequencer.setPluginBypassed(instrument, true);
                        audition(track.trackIndex, preview, event);
                        error = undefined;
                    } catch (e) {
                        error = e instanceof Error ? e.message : String(e);
                    }
                    repl.patchBrowser.answer(error);
                }
            }
        } finally {
            if (preview.instanceId !== undefined) uapmd.instance(preview.instanceId).dispose();
            if (instrument >= 0) uapmd.sequencer.setPluginBypassed(instrument, bypassed);
        }
    }

    return {use, browse};
})();
