// Thin bindings: original crate/function names, JSON byte arrays, JS exceptions.
function __replCall(api, args) {
    var reply = JSON.parse(__repl_host_call(JSON.stringify(Object.assign({api: api}, args))));
    if (!reply.ok) throw new Error(api + ': ' + reply.error);
    return reply.value;
}

// Explicit builtin/... or user/... paths; reread and evaluate on every call.
// Global evaluation keeps included scripts independent of the caller's locals.
// Libraries can end in an IIFE returning their public functions.
globalThis.include = function(name) {
    try {
        return (0, eval)(__replCall('snippet.source', {name:name}));
    } catch (error) {
        if (!(error instanceof Error)) error = new Error(String(error));
        error.message = 'include(' + JSON.stringify(name) + '): ' + error.message;
        throw error;
    }
};
globalThis.cmrt_chord = {
    resolve_chord_or_mml: function(input) {
        return __replCall('cmrt_chord.resolve_chord_or_mml', {input: input});
    }
};
globalThis.mmlabc_to_smf = {
    mml_to_smf_bytes: function(mml) {
        return __replCall('mmlabc_to_smf.mml_to_smf_bytes', {mml: mml});
    },
    mml_preprocessor: {extract_embedded_json: function(mml) {
        return __replCall('mmlabc_to_smf.mml_preprocessor.extract_embedded_json', {mml:mml});
    }},
    pass1_parser: {parse_mml: function(mml) {
        return __replCall('mmlabc_to_smf.pass1_parser.parse_mml', {mml:mml});
    }},
    pass2_ast: {tokens_to_ast: function(tokens) {
        return __replCall('mmlabc_to_smf.pass2_ast.tokens_to_ast', {tokens:tokens});
    }},
    pass3_events: {ast_to_events: function(ast, use_drum_channel_for_128) {
        return __replCall('mmlabc_to_smf.pass3_events.ast_to_events',
            {ast:ast, use_drum_channel_for_128:use_drum_channel_for_128});
    }},
    pass4_midi: {
        save_midi_file: function(bytes, filepath) {
            __replCall('mmlabc_to_smf.pass4_midi.save_midi_file', {bytes: bytes, filepath: filepath});
        }
    }
};
// Creates only the directory; returns a new path without creating or deleting a file.
function getGeneratedMidiPath() {
    return __replCall('generatedMidiPath', {});
}

globalThis.repl = {text: function(key, values) { return __replCall('text', {key:key, values:values}); }};

repl.processEvents = function() { return __replCall('control.processEvents', {}); };
repl.audio = {isProcessing: function() { return __replCall('audio.isProcessing', {}); }};
// The track's volume as a linear gain (1 keeps the level, 0 silences it), as uapmd's track slider sets it.
// setVolume also takes decibels as a string such as '-3dB'.
// Both return the volume the track now has, as a linear gain.
repl.track = {
    volume: function(trackIndex) { return __replCall('track.volume', {trackIndex:trackIndex}); },
    setVolume: function(trackIndex, volume) {
        if (typeof volume === 'string') {
            const decibels = /^\s*([+-]?(?:\d+\.?\d*|\.\d+))\s*dB\s*$/i.exec(volume);
            if (!decibels) throw new Error('Invalid track volume: ' + volume);
            volume = Math.pow(10, Number(decibels[1]) / 20);
        }
        if (!(Number.isFinite(volume) && volume >= 0)) throw new Error('Invalid track volume');
        return __replCall('track.setVolume', {trackIndex:trackIndex, volume:volume});
    }
};
// The instance whose output the player's scope shows; -1 for none.
repl.scope = {
    target: function() { return __replCall('scope.target', {}); },
    setTarget: function(instanceId) { return __replCall('scope.setTarget', {instanceId:instanceId}); }
};
// Native observations for test scripts. observe(): the master output's latest
// 256 samples as {finite, peak, rms} with {blocks, engineEnabled, dispatcherPlaying}.
// notes(): pitches the scope target's plugin input holds now.
// loopTrace(start): {events:[{frame, note, on, instance}], overflow, programChanges, timing}
// of plugin input since the last loopTrace(true); start needs a stopped device.
// loopChecks(): the native loop-playback regression; throws on a failure.
// pluginState(instanceId): the instance's whole state as a hex string, within five seconds.
repl.test = {
    observe: function() { return __replCall('test.observe', {}); },
    notes: function() { return __replCall('test.notes', {}); },
    loopTrace: function(start) { return __replCall('test.loopTrace', {start:start === true}); },
    loopChecks: function() { return __replCall('test.loopChecks', {}); },
    pluginState: function(instanceId) { return __replCall('test.pluginState', {instanceId:instanceId}); }
};
// The player's catalog patch `patch` (its display path) as a state file for `pluginId`.
repl.patchState = function(options) {
    return __replCall('patchState', {format:options.format, pluginId:options.pluginId, patch:options.patch});
};
// The effect preset file `preset` (relative to the plugin's root) with the player's
// selector `value` inside it, as a state file for `pluginId`.
repl.effectState = function(options) {
    return __replCall('effectState', {format:options.format, pluginId:options.pluginId, preset:options.preset, value:options.value});
};

// cat-plugin-player's patch or effect browser. poll() returns what arrived since the last call:
// {kind:'selected', format, pluginId, plugin, patch, stateFile} or {kind:'closed', error}.
// answer(error) replies to the last selected; the player offers the next one after it.
// open({track, kind, cell}) names the browsed track, which the cell of the chosen preset
// sets; kind is 'patch' (the default) or 'effect'; cell:false writes no cell.
repl.patchBrowser = {
    open: function(options) {
        return __replCall('patchBrowser.open', {track:options.track, kind:options.kind, cell:options.cell});
    },
    poll: function() { return __replCall('patchBrowser.poll', {}); },
    answer: function(error) { return __replCall('patchBrowser.answer', {error:error === undefined ? null : error}); }
};

repl.ui = {
    show: function(definition) { return __replCall('ui.show', {definition:definition}); },
    close: function(id) { return __replCall('ui.close', {id:id}); }
};

// Only the missing native timing primitives; composition and part names stay in script.
repl.timeline = {
    configureLoop: function(beats, bpm) {
        return __replCall('timeline.loop.configure', {beats:beats, bpm:bpm});
    },
    publishLoopClips: function(parts, beats, bpm) {
        if (beats !== undefined && !(Number.isFinite(beats) && beats > 0)) throw new Error('Invalid loop length');
        if (bpm !== undefined && !(Number.isFinite(bpm) && bpm > 0)) throw new Error('Invalid loop tempo');
        return __replCall('timeline.loop.publish', {parts:parts, beats:beats, bpm:bpm});
    },
    loopStatus: function() { return __replCall('timeline.loop.status', {}); },
    clearLoop: function() { return __replCall('timeline.loop.clear', {}); }
};

// Notes of the clip `clipId` (a uapmd.timeline clip id), named by getClipNotes' noteId.
// cut returns how many notes it cut; paste puts the clipboard's first note at `tick`
// and returns the pasted notes' noteIds. Both save the clip.
repl.pianoRoll = {
    cut: function(clipId, noteIds) {
        if (!Array.isArray(noteIds) || !noteIds.every(id => typeof id === 'string')) throw new Error('Invalid note ids');
        return __replCall('pianoRoll.cut', {clipId:clipId, noteIds:noteIds});
    },
    paste: function(clipId, tick) {
        if (!(Number.isInteger(tick) && tick >= 0)) throw new Error('Invalid paste tick');
        return __replCall('pianoRoll.paste', {clipId:clipId, tick:tick});
    }
};
