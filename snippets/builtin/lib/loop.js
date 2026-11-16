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
        return {trackIndex:trackIndex, filepath:path, beats:beats, mml:mml};
    }

    // Stage standard UAPMD clips before removing old ones, so failed imports
    // leave the existing performance intact. No scheduler or boundary reservation.
    function publishLoopParts(next, bpm, replaceAll = false) {
        if (!(Number.isFinite(bpm) && bpm > 0)) throw new Error('Invalid tempo');
        const entries = Object.entries(next);
        const beats = entries.reduce((length, [, p]) => Math.max(length, p.beats), 0);
        if (!(Number.isFinite(beats) && beats > 0)) throw new Error('Invalid loop length');
        const samples = Math.round(beats * 60 / bpm * uapmd.sequencer.getSampleRate());
        if (!(Number.isSafeInteger(samples) && samples > 0)) throw new Error('Invalid loop length');
        const tracks = include('builtin/lib/tracks.js');
        const seen = [];
        for (const [, part] of entries) {
            tracks.info(part.trackIndex);
            if (seen.includes(part.trackIndex)) throw new Error('Combine parts on the same track in script');
            seen.push(part.trackIndex);
        }
        const masterId = uapmd.timeline.getMasterTrackId();
        const masterIds = () => uapmd.timeline.getClips(masterId).map(c => c.clipId);
        const initialMasters = masterIds();
        const added = [];
        const published = {};
        try {
            for (const [name, part] of entries) {
                // Imported master tempo events, rather than the transient state field,
                // are UAPMD's timing authority. Supply the caller's default tempo in the SMF.
                const filepath = part.mml === undefined ? part.filepath : midi.writeMml('t' + bpm + ' ' + part.mml);
                const before = masterIds();
                const clip = midi.addFile(part.trackIndex, filepath);
                added.push(clip.clipId);
                published[name] = {...part, clipId:clip.clipId, masterClipIds:masterIds().filter(id => !before.includes(id))};
            }
        } catch (error) {
            for (const clipId of added) uapmd.timeline.removeClip(clipId);
            for (const clipId of masterIds().filter(id => !initialMasters.includes(id)))
                uapmd.timeline.removeClip(clipId);
            throw error;
        }
        repl.sequence.clear();
        const previous = Object.values(globalThis.loopParts || {}).map(p => p.clipId);
        for (const track of uapmd.sequencer.getTrackInfos()) {
            for (const clip of uapmd.timeline.getClips(track.trackId)) {
                if (!added.includes(clip.clipId) && (replaceAll || previous.includes(clip.clipId))) {
                    if (!uapmd.timeline.removeClip(clip.clipId))
                        throw new Error(repl.text('snippet_14', {clipId:clip.clipId}));
                }
            }
        }
        for (const part of Object.values(globalThis.loopParts || {})) {
            for (const id of part.masterClipIds || []) {
                if (masterIds().includes(id) && !uapmd.timeline.removeClip(id))
                    throw new Error(repl.text('snippet_14', {clipId:id}));
            }
        }
        uapmd.timeline.setTempo(bpm);
        repl.timeline.setLoop(0, samples);
        globalThis.loopParts = published;
    }

    // Counterpart of tutorial play(): ordinary clips and loop clips share one timeline.
    function playLoopParts(next, bpm) {
        publishLoopParts(next, bpm, true);
        const plugins = include('builtin/lib/plugins.js');
        for (const part of Object.values(next)) {
            const instrument = plugins.instrumentOn(part.trackIndex);
            if (instrument >= 0) uapmd.sequencer.setPluginBypassed(instrument, false);
        }
        uapmd.audio.setEngineEnabled(true);
        if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
    }

    return {loopPhraseBeats, prepareLoopPart, publishLoopParts, playLoopParts};
})();
