// Shared preparation for the try-* snippets. Loading only defines functions.
(() => {
    var plugins = include('builtin/lib/plugins.js');
    var midi = include('builtin/lib/midi.js');
    var playback;
    function waitFor(ready) {
        var deadline = Date.now() + 10000;
        while (!ready()) {
            if (Date.now() >= deadline) throw new Error('Demo preparation timed out');
            repl.processEvents();
        }
    }
    function stop() {
        // Stopping an already inactive engine leaves a flush for the next block.
        // Only request a transport stop when it is actually playing.
        if (uapmd.timeline.getState().isPlaying) uapmd.sequencer.stopPlayback();
        uapmd.audio.setEngineEnabled(false);
        waitFor(() => !repl.audio.isProcessing());
    }
    function reset() {
        stop();
        repl.timeline.clearLoop();
        // Without the map, the next loop publication takes the first-publication path.
        delete globalThis.loopParts;
        // @text demo_isolate
        for (var track of uapmd.sequencer.getTrackInfos()) {
            for (var node of track.nodes.filter(n => n.instanceId >= 0)) {
                uapmd.sequencer.setPluginBypassed(node.instanceId, true);
            }
        }
    }

    function activate(id) {
        var track = plugins.trackFor(id);
        uapmd.sequencer.setPluginBypassed(id, false);
        return {instanceId:id, trackIndex:track.trackIndex};
    }

    // Let the plugin load the preset's settings. Do not copy factory volume
    // numbers into scripts just to match Init Saw exactly: those opaque values
    // hide intent, overwrite edits, and are not a justified safety limit.
    // play() must never reload a preset or write volume; init() uses normal loading.
    function preparePreset(instanceId, presetName) {
        var synth = uapmd.instance(instanceId);
        restoreBypassControls(instanceId);
        var preset = synth.getPresets().find(p => p.name === presetName);
        if (!preset) throw new Error(repl.text('snippet_09'));
        var presetJob = synth.loadPreset(preset.index);
        waitFor(() => {
            var job = uapmd.mutations.getJob(presetJob.jobId);
            if (job.state === 'failed') throw new Error(job.error);
            return job.state === 'completed';
        });
        return {...activate(instanceId), presetJobId:presetJob.jobId};
    }

    // The track's instrument becomes the plugin (replacing another one), with
    // its parameters (or only its levels) back at the plugin's defaults.
    function prepareInstrument(trackIndex, format, plugin, levelsOnly) {
        const id = include('builtin/lib/patch.js').use({format, plugin, track:trackIndex}).instanceId;
        const synth = uapmd.instance(id);
        for (const p of synth.getParameters().filter(p => !p.isReadonly && (!levelsOnly || /volume|level|gain/i.test(p.name))))
            synth.setParameterValue(p.id, p.defaultValue);
        return activate(id);
    }

    // A track whose connections were edited leaves a new plugin unconnected:
    // put it between what reached the track's output and the output.
    function joinBeforeOutput(trackIndex, effectId) {
        const connections = plugins.connectionsOf(trackIndex);
        const isEffect = e => e.type === 'plugin' && e.instanceId === effectId;
        if (connections.some(c => isEffect(c.source) || isEffect(c.target))) return;
        const check = result => { if (!result.success) throw new Error(result.error); };
        const buses = new Set();
        for (const c of connections.filter(c => c.busType === 'audio' && c.target.type === 'graphOutput')) {
            check(uapmd.timeline.disconnectTrackGraphConnection(trackIndex, c.id));
            const source = c.source.type === 'plugin' ? plugins.pluginEnd(c.source.instanceId, c.source.busIndex)
                : {type:c.source.type, instanceId:c.source.instanceId, nodeId:c.source.nodeId || '', busIndex:c.source.busIndex};
            check(uapmd.timeline.connectTrackGraph(trackIndex, 'audio', source, plugins.pluginEnd(effectId, c.target.busIndex)));
            buses.add(c.target.busIndex);
        }
        if (buses.size === 0) buses.add(0);
        for (const bus of buses)
            check(uapmd.timeline.connectTrackGraph(trackIndex, 'audio', plugins.pluginEnd(effectId, bus),
                {type:'graphOutput', instanceId:-1, nodeId:'', busIndex:bus}));
    }

    // New effects use plugin defaults; reused effects keep the user's edits.
    // If a requested preset cannot be loaded through the available API, report
    // that limitation. Do not replace it with an unexplained dump of raw values.
    function prepareEffect(trackIndex, format, pluginId) {
        var track = uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === trackIndex);
        if (!track) throw new Error(repl.text('snippet_15'));
        var effect = track.nodes.find(n => n.pluginId === pluginId);
        var effectId = effect ? effect.instanceId : uapmd.instancing.create(format, pluginId, trackIndex);
        if (effectId < 0) throw new Error(repl.text('snippet_17'));
        if (!effect) joinBeforeOutput(trackIndex, effectId);
        var fx = uapmd.instance(effectId);
        uapmd.sequencer.setPluginBypassed(effectId, false);
        return fx;
    }

    function replaceMml(trackIndex, mml, bpm) {
        // @text demo_replace
        for (var clip of uapmd.timeline.getClips(trackIndex)) {
            if (!uapmd.timeline.removeClip(trackIndex, clip.clipId))
                throw new Error(repl.text('snippet_14', {clipId:clip.clipId}));
        }
        uapmd.timeline.setTempo(bpm);
        var files = globalThis.tutorialMidiFiles || (globalThis.tutorialMidiFiles = {});
        if (!files[mml]) {
            files[mml] = midi.writeMml(mml);
        }
        var added = midi.addFile(trackIndex, files[mml]);
        return {clipId:added.clipId};
    }

    function restoreBypassControls(instanceId) {
        var synth = uapmd.instance(instanceId);
        for (var control of synth.getParameters()) {
            if (['FX Chain Bypass', 'FX Disable', 'Bypass Surge XT'].includes(control.name))
                synth.setParameterValue(control.id, control.defaultValue);
        }
    }

    function init(options) {
        if (!Number.isFinite(options.bpm) || options.bpm <= 0)
            throw new Error('Demo tempo must be positive');
        plugins.trackFor(options.instanceId);
        reset();
        var synth;
        if (options.preset !== undefined) synth = preparePreset(options.instanceId, options.preset);
        else {
            restoreBypassControls(options.instanceId);
            synth = activate(options.instanceId);
        }
        playback = {instanceId:synth.instanceId, bpm:options.bpm};
        return synth;
    }

    function play(input) {
        if (!playback) throw new Error('Call demo.init before demo.play');
        var mml = cmrt_chord.resolve_chord_or_mml(input).mml;
        var synth = plugins.trackFor(playback.instanceId);
        stop();
        repl.timeline.clearLoop();
        delete globalThis.loopParts;
        var clip = replaceMml(synth.trackIndex, mml, playback.bpm);
        uapmd.sequencer.setPluginBypassed(playback.instanceId, false);
        // startPlayback owns position zero and preroll. A jump here requests a
        // new stop flush, which can discard the first note after the engine restarts.
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.startPlayback();
        return {...playback, trackIndex:synth.trackIndex, ...clip};
    }

    return {reset, preparePreset, prepareInstrument, prepareEffect, replaceMml, restoreBypassControls, init, play};
})();
