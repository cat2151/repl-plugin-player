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

    function prepareInstrument(stateKey, format, pluginId, levelsOnly) {
        var tracks = uapmd.sequencer.getTrackInfos();
        var saved = globalThis[stateKey];
        var fresh = false;
        if (!saved || !tracks.some(t => t.nodes.some(n => n.instanceId === saved.id))) {
            fresh = true;
            var id = uapmd.instancing.create(format, pluginId, -1);
            if (id < 0) throw new Error(repl.text('snippet_02'));
            var synth = uapmd.instance(id);
            saved = globalThis[stateKey] = {id, values:synth.getParameters()
                .filter(p => !p.isReadonly && (!levelsOnly || /volume|level|gain/i.test(p.name)))
                .map(p => [p.id, synth.getParameterValue(p.id)])};
        }
        if (!levelsOnly || !fresh)
            for (var value of saved.values) uapmd.instance(saved.id).setParameterValue(value[0], value[1]);
        return activate(saved.id);
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
