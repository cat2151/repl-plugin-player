(() => {
    var midi = include('builtin/lib/midi.js');
    // Converter markers exist only in this timing copy, never in the playing SMF.
    function loopPhraseBeats(input) {
        var resolved = cmrt_chord.resolve_chord_or_mml(input);
        var source = mmlabc_to_smf.mml_preprocessor.extract_embedded_json(resolved.mml);
        var tokens = mmlabc_to_smf.pass1_parser.parse_mml(source.remaining_mml);
        var ast = mmlabc_to_smf.pass2_ast.tokens_to_ast(tokens);
        var channels = [];
        ast.notes.forEach(n => {
            var channel = n.channel === undefined ? null : n.channel;
            if (channels.indexOf(channel) < 0) channels.push(channel);
        });
        channels.forEach(channel => ast.notes.push({type:'program_change', pitch:0, name:'', channel:channel}));
        var events = mmlabc_to_smf.pass3_events.ast_to_events(ast, true);
        var ticks = events.reduce((end, e) => Math.max(end, e.time), 0);
        if (!(ticks > 0)) throw new Error('Loop phrase must have positive length');
        return ticks / 480;
    }

    function prepareLoopPart(trackIndex, input) {
        var mml = cmrt_chord.resolve_chord_or_mml(input).mml;
        var beats = loopPhraseBeats(mml);
        var path = midi.writeMml(mml);
        return {trackIndex:trackIndex, filepath:path, beats:beats};
    }

    function publishLoopParts(next, bpm) {
        var parts = Object.keys(next).map(k => next[k]);
        var beats = parts.reduce((length, p) => Math.max(length, p.beats), 0);
        // First publication needs initialized native timing. Later calls keep the cycle running.
        if (!globalThis.loopParts && uapmd.timeline.getState().isPlaying) {
            uapmd.sequencer.stopPlayback();
            // Existing shutdown/restart drains the stop flush before the first loop note.
            uapmd.audio.setEngineEnabled(false);
        }
        repl.timeline.publishLoopClips(parts, beats, bpm);
        globalThis.loopParts = next;
    }

    // Counterpart of tutorial play(): only the loop sounds afterwards.
    function playLoopParts(next, bpm) {
        var clips = [];
        for (var track of uapmd.sequencer.getTrackInfos()) {
            for (var clip of uapmd.timeline.getClips(track.trackId)) clips.push(clip.clipId);
        }
        // A running loop is updated at its boundary; ordinary clips force a fresh entry instead.
        if (clips.length > 0 || !globalThis.loopParts) {
            if (uapmd.timeline.getState().isPlaying) {
                uapmd.sequencer.stopPlayback();
                uapmd.audio.setEngineEnabled(false);
            }
            if (globalThis.loopParts) {
                repl.timeline.clearLoop();
                delete globalThis.loopParts;
            }
            for (var c of clips) {
                if (!uapmd.timeline.removeClip(c))
                    throw new Error(repl.text('snippet_14', {clipId:c}));
            }
        }
        publishLoopParts(next, bpm);
        var tracks = Object.keys(next).map(k => next[k].trackIndex);
        // Only the instrument, as play() does; effects keep their bypass state.
        const plugins = include('builtin/lib/plugins.js');
        for (var trackIndex of uapmd.sequencer.getTrackInfos().map((t, i) => i).filter(i => tracks.indexOf(i) >= 0)) {
            const instrument = plugins.instrumentOn(trackIndex);
            if (instrument >= 0) uapmd.sequencer.setPluginBypassed(instrument, false);
        }
        uapmd.audio.setEngineEnabled(true);
        if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
    }

    return {loopPhraseBeats, prepareLoopPart, publishLoopParts, playLoopParts};
})();
