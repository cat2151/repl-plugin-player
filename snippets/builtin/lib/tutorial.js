// Shared preparation for the try-* snippets. Loading only defines functions.
(() => {
    var plugins = include('builtin/lib/plugins.js');
    var midi = include('builtin/lib/midi.js');
    var tracks = include('builtin/lib/tracks.js');
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
        // Discard script references to the previous clips.
        delete globalThis.loopParts;
        // @text demo_isolate
        for (var track of uapmd.sequencer.getTrackInfos()) {
            for (var node of track.nodes.filter(n => n.instanceId >= 0)) {
                uapmd.sequencer.setPluginBypassed(node.instanceId, true);
            }
        }
    }

    function activate(id) {
        var trackIndex = plugins.trackFor(id);
        uapmd.sequencer.setPluginBypassed(id, false);
        return {instanceId:id, trackIndex};
    }

    // Do not copy factory volume numbers into scripts: those opaque values hide
    // intent, overwrite edits, and are not a justified safety limit.
    // play() must never reload a patch or write volume.
    function unbypass(instanceId) {
        restoreBypassControls(instanceId);
        return activate(instanceId);
    }

    // The track's instrument becomes the plugin (replacing another one) with
    // the patch, or, without one, with its parameters (or only its levels)
    // back at the plugin's defaults. Options are patch.js use()'s plus levelsOnly.
    function prepareInstrument(options) {
        const {levelsOnly, ...use} = options;
        const id = include('builtin/lib/patch.js').use(use).instanceId;
        if (use.patch === undefined) {
            const synth = uapmd.instance(id);
            for (const p of synth.getParameters().filter(p => !p.isReadonly && (!levelsOnly || /volume|level|gain/i.test(p.name))))
                synth.setParameterValue(p.id, p.defaultValue);
        }
        return unbypass(id);
    }

    // New effects use plugin defaults; reused effects keep the user's edits.
    // If a requested preset cannot be loaded through the available API, report
    // that limitation. Do not replace it with an unexplained dump of raw values.
    function prepareEffect(trackIndex, format, pluginId) {
        var track = uapmd.sequencer.getTrackInfos()[trackIndex];
        if (!Number.isInteger(trackIndex) || !track) throw new Error(repl.text('snippet_15'));
        var effect = track.nodes.find(n => n.pluginId === pluginId);
        var effectId = effect ? effect.instanceId : uapmd.instancing.create(format, pluginId, track.trackId);
        if (effectId < 0) throw new Error(repl.text('snippet_17'));
        if (!effect) include('builtin/lib/effect.js').joinBeforeOutput(trackIndex, effectId);
        var fx = uapmd.instance(effectId);
        uapmd.sequencer.setPluginBypassed(effectId, false);
        return fx;
    }

    function replaceMml(trackIndex, mml, bpm) {
        // @text demo_replace
        for (var clip of uapmd.timeline.getClips(tracks.idOf(trackIndex))) {
            if (!uapmd.timeline.removeClip(clip.clipId))
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
        var synth = unbypass(options.instanceId);
        playback = {instanceId:synth.instanceId, bpm:options.bpm};
        return synth;
    }

    function play(input) {
        if (!playback) throw new Error('Call demo.init before demo.play');
        var mml = cmrt_chord.resolve_chord_or_mml(input).mml;
        var trackIndex = plugins.trackFor(playback.instanceId);
        stop();
        repl.timeline.clearLoop();
        delete globalThis.loopParts;
        var clip = replaceMml(trackIndex, mml, playback.bpm);
        uapmd.sequencer.setPluginBypassed(playback.instanceId, false);
        // startPlayback owns position zero and preroll. A jump here requests a
        // new stop flush, which can discard the first note after the engine restarts.
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.startPlayback();
        return {...playback, trackIndex, ...clip};
    }

    return {reset, stop, prepareInstrument, prepareEffect, replaceMml, restoreBypassControls, init, play};
})();
