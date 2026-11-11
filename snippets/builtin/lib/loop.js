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

    return {loopPhraseBeats, prepareLoopPart, publishLoopParts};
})();
