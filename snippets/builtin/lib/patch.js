// Select a plugin by the name the user reads and a patch by its path in the player's catalog,
// or take patches from the player's browser. Loading only defines functions.
(() => {
    const plugins = include('builtin/lib/plugins.js');
    const loadState = plugins.loadState;
    const tracks = include('builtin/lib/tracks.js');
    const pluginEnd = instanceId => plugins.pluginEnd(instanceId);

    // Wires `next` where `previous` was: the track's input reaches it, and it
    // feeds what `previous` fed (the track's effects, or the output). Every
    // connection is laid again, because a plugin created on the track joins
    // the chain behind its effects. Other connections stay as `before` had them.
    function wireInstead(trackIndex, previous, next, before) {
        const trackId = tracks.idOf(trackIndex);
        for (const c of plugins.connectionsOf(trackIndex)) {
            const result = uapmd.timeline.disconnectTrackGraphConnection(trackId, c.id);
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
            const result = uapmd.timeline.connectTrackGraph(trackId, busType, source, target);
            if (!result.success) throw new Error(result.error);
        }
    }

    // Whether the track's instrument `previous` is of the plugin `entry`.
    function reusable(trackIndex, previous, entry) {
        const node = tracks.info(trackIndex).nodes.find(n => n.instanceId === previous);
        return node !== undefined && node.format === entry.format && node.pluginId === entry.pluginId;
    }

    function instrumentFor(trackIndex, entry, reuseOnly = false) {
        const before = plugins.connectionsOf(trackIndex);
        const previous = plugins.instrumentOn(trackIndex);
        if (reusable(trackIndex, previous, entry)) return previous;
        if (reuseOnly) throw new Error('Prepared plugin instance is missing: ' + entry.pluginId);
        const next = uapmd.instancing.create(entry.format, entry.pluginId, tracks.idOf(trackIndex));
        if (next < 0) throw new Error(repl.text('snippet_02'));
        wireInstead(trackIndex, previous, next, before);
        if (previous >= 0 && repl.scope.target() === previous) repl.scope.setTarget(next);
        if (previous >= 0) {
            uapmd.instance(previous).dispose();
        }
        return next;
    }

    function use(options) {
        const trackIndex = options.track;
        if (!Number.isInteger(trackIndex)) throw new Error(repl.text('patch_use_02'));
        tracks.info(trackIndex);
        const entry = options.pluginId === undefined ? plugins.catalogEntry(options.format, options.plugin)
            : Array.from({length:uapmd.catalog.getCount()}, (_, i) => uapmd.catalog.getPluginAt(i))
                .find(p => p.format === options.format && p.pluginId === options.pluginId);
        if (!entry) throw new Error('Catalog plugin is not installed: ' + options.pluginId);
        const previous = plugins.instrumentOn(trackIndex);
        // A patch that cannot be restored fails before an instance is created.
        if (options.patch !== undefined && !reusable(trackIndex, previous, entry)) plugins.checkPatch(entry, options.patch);
        const instanceId = instrumentFor(trackIndex, entry, options.reuseOnly);
        if (options.patch !== undefined) plugins.loadPatch(instanceId, instanceId !== previous, entry, options.patch);
        uapmd.sequencer.setPluginBypassed(instanceId, false);
        return {instanceId, trackIndex};
    }

    // The sound being auditioned: one instance, added to the browsed track.
    function audition(trackIndex, preview, event) {
        if (preview.instanceId === undefined || preview.format !== event.format || preview.pluginId !== event.pluginId) {
            const id = uapmd.instancing.create(event.format, event.pluginId, tracks.idOf(trackIndex));
            if (id < 0) throw new Error(repl.text('snippet_02'));
            plugins.joinAlone(trackIndex, id);
            if (preview.instanceId !== undefined) uapmd.instance(preview.instanceId).dispose();
            Object.assign(preview, {instanceId:id, format:event.format, pluginId:event.pluginId});
        }
        loadState(preview.instanceId, event.stateFile);
        uapmd.sequencer.setPluginBypassed(preview.instanceId, false);
    }

    // Auditions each patch chosen in the player's browser on the track while it
    // keeps playing; closing the browser brings back the track's own sound.
    function browse(options) {
        const trackIndex = options.track;
        tracks.info(trackIndex);
        // The track's instrument stays untouched, only bypassed.
        const instrument = plugins.instrumentOn(trackIndex);
        var bypassed = instrument >= 0 && uapmd.sequencer.isPluginBypassed(instrument);
        var preview = {};
        try {
            include('builtin/lib/browser.js').run({track:trackIndex}, event => {
                if (instrument >= 0) uapmd.sequencer.setPluginBypassed(instrument, true);
                audition(trackIndex, preview, event);
            });
        } finally {
            if (preview.instanceId !== undefined) uapmd.instance(preview.instanceId).dispose();
            if (instrument >= 0) uapmd.sequencer.setPluginBypassed(instrument, bypassed);
        }
    }

    // Each patch chosen in the player's browser becomes the track's instrument
    // at once and stays after the browser closes. options: {track, cell};
    // `cell:false` writes no cell for the last patch.
    function change(options) {
        const trackIndex = options.track;
        tracks.info(trackIndex);
        include('builtin/lib/browser.js').run({track:trackIndex, cell:options.cell}, event => {
            const instanceId = instrumentFor(trackIndex, {format:event.format, pluginId:event.pluginId});
            loadState(instanceId, event.stateFile);
            uapmd.sequencer.setPluginBypassed(instanceId, false);
        });
    }

    function random({tracks: targets, role, kind, attempts = 8, exclude = [], instruments, load = use}) {
        if (!Number.isInteger(attempts) || attempts < 1) throw new Error('Invalid patch selection attempts');
        if (new Set(targets).size !== targets.length) throw new Error('Expected distinct tracks');
        targets.forEach(track => tracks.info(track));
        const installed = Array.from({length:uapmd.catalog.getCount()}, (_, i) => uapmd.catalog.getPluginAt(i));
        const candidates = repl.patchCatalog({role, kind}).filter(p => installed.some(
            plugin => plugin.format === p.format && plugin.pluginId === p.pluginId)
            && (!instruments || instruments.some(plugin => plugin.format === p.format && plugin.pluginId === p.pluginId)));
        const key = p => JSON.stringify([p.format,p.pluginId,p.patch]);
        const excluded = new Set(exclude.map(key));
        const unique = [...new Map(candidates.filter(p => !excluded.has(key(p))).map(p => [key(p),p])).values()];
        if (unique.length < targets.length) throw new Error('Not enough distinct catalog patches for ' + role + ' / ' + kind);
        return targets.map(track => {
            const failures = [];
            for (let attempt = 0; attempt < attempts && unique.length; ++attempt) {
                const selected = unique.splice(Math.floor(Math.random() * unique.length), 1)[0];
                try { return {...selected, ...load({...selected, track})}; }
                catch (error) {
                    const failure = selected.plugin + ' / ' + selected.patch + ': ' + error.message;
                    failures.push(failure);
                    console.log(failure);
                }
            }
            throw new Error('Could not load ' + role + ' / ' + kind + ':\n' + failures.join('\n'));
        });
    }

    return {use, browse, change, random};
})();
