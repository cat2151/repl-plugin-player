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
    note_progression: function(input) { return __replCall('cmrt_chord.note_progression', {input:input}); },
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

repl.processEvents = function() {
    const result = __replCall('control.processEvents', {});
    const errors = __replTick();
    if (errors.length) throw new Error(errors.join('\n'));
    return result;
};
// Control-thread updates only. Missed periods coalesce; audio has its own clock.
(() => {
    const tasks = new Map();
    let ticking = false;
    repl.tasks = {
        every(id, milliseconds, callback) {
            if (!Number.isFinite(milliseconds) || milliseconds < 10 || typeof callback !== 'function')
                throw new Error('Expected a callback and period >= 10ms');
            tasks.set(id, {milliseconds, callback, next:Date.now() + milliseconds});
        },
        cancel(id) { tasks.delete(id); }
    };
    globalThis.__replTick = () => {
        if (ticking) return [];
        ticking = true;
        const errors = [];
        try {
            const now = Date.now();
            for (const [id, task] of [...tasks]) {
                if (tasks.get(id) !== task || now < task.next) continue;
                task.next = now + task.milliseconds;
                try { task.callback(); }
                catch (error) { tasks.delete(id); errors.push(id + ': ' + String(error)); }
            }
        } finally { ticking = false; }
        return errors;
    };
})();
repl.audio = {
    isProcessing: function() { return __replCall('audio.isProcessing', {}); },
    // Auto gain for these live pad instances. An empty list turns it off.
    setPadAutoGain: function(instances) { return __replCall('audio.setPadAutoGain', {instances}); }
};
// UAPMD's self-contained archive, including MIDI and plugin states.
repl.project = {
    directory: function() { return __replCall('projectsDirectory', {}); },
    saveArchive: function(filepath) { return __replCall('project.saveArchive', {filepath:filepath}); },
    // Returns after UAPMD has restored the timeline and plugin states.
    load: function(filepath) {
        globalThis.performancePadSession?.();
        include('builtin/lib/evolve-mode.js').end();
        repl.sequence.clear();
        const previousId = repl.scope.target();
        const previousTracks = uapmd.sequencer.getTrackInfos();
        const previousTrack = previousId < 0 ? undefined
            : previousTracks.find(track => track.nodes.some(node => node.instanceId === previousId));
        const previousNode = previousTrack
            && previousTrack.nodes.find(node => node.instanceId === previousId);
        const path = __replCall('project.load', {filepath:filepath});
        delete globalThis.loopParts;
        // Instances are recreated, but document track/node identities survive a load.
        const tracks = uapmd.sequencer.getTrackInfos();
        const track = previousTrack && tracks.find(track => track.trackId === previousTrack.trackId);
        const node = track && track.nodes.find(node => node.nodeId === previousNode.nodeId && node.instanceId >= 0);
        let target = node ? node.instanceId : -1;
        if (target < 0) {
            const plugins = include('builtin/lib/plugins.js');
            for (let track = 0; track < tracks.length && target < 0; ++track)
                target = plugins.instrumentOn(track);
        }
        repl.scope.setTarget(target);
        return path;
    }
};
// read: the whole file as byte numbers. list: one directory level as [{name, directory}] in name order.
repl.file = {
    read: function(filepath) { return __replCall('file.read', {filepath:filepath}); },
    list: function(directory) { return __replCall('file.list', {directory:directory}); }
};
// Inflates the zlib stream at the start of `bytes`; bytes after the stream's end are ignored.
repl.zlib = {decompress: function(bytes) { return __replCall('zlib.decompress', {bytes:bytes}); }};
// The repository this exe was built in, or null when the exe runs outside one.
repl.sourceRoot = function() { return __replCall('sourceRoot', {}); };
// Windows' "minimum processor state" (AC and DC) of the active power scheme, in percent.
repl.power = {setMinProcessorState: function(percent) {
    __replCall('power.setMinProcessorState', {percent:percent});
}};
// onExit(name, fn) runs fn once when the app ends normally (not after a crash or a kill);
// a later call with the same name replaces it, and fn null removes it. In the GUI the
// window closes after the dialogs that fn shows are closed.
(() => {
    const handlers = new Map();
    repl.app = {onExit: function(name, fn) {
        if (fn === null) handlers.delete(name);
        else if (typeof fn === 'function') handlers.set(name, fn);
        else throw new Error('repl.app.onExit: fn must be a function or null');
    }};
    // The host calls this when the app is ending; a second call finds nothing to run.
    globalThis.__replRunExitHandlers = function() {
        const pending = [...handlers.values()];
        handlers.clear();
        const errors = [];
        for (const fn of pending) {
            try { fn(); } catch (error) { errors.push(error instanceof Error ? error.message : String(error)); }
        }
        if (errors.length > 0) throw new Error(errors.join('\n'));
    };
})();
// bind(key, fn): pressing key (written as in keybinds.toml) calls fn wherever the focus
// is, as a run whose result and error the console shows; binding the key again replaces
// fn. A key that a command of keybinds.toml uses is an error.
// config(): keybinds.toml as read at startup: each command's key, and scripts ([scripts]).
(() => {
    const handlers = new Map();
    repl.keys = {
        bind: function(key, fn) {
            if (typeof fn !== 'function') throw new Error('repl.keys.bind: fn must be a function');
            handlers.set(__replCall('keys.bind', {key:key}), fn);
        },
        config: function() { return __replCall('keys.config', {}); }
    };
    // The host calls this with the name keys.bind returned, when that key is pressed.
    globalThis.__replRunKey = function(name) {
        const fn = handlers.get(name);
        if (!fn) throw new Error('repl.keys: ' + name + ' is not bound');
        return fn();
    };
})();
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
// pluginState(instanceId): the instance's whole state as a hex string, within five seconds.
// instances(): the ids of every plugin instance the host holds, unpublished graphs' included.
repl.test = {
    observe: function() { return __replCall('test.observe', {}); },
    notes: function() { return __replCall('test.notes', {}); },
    loopTrace: function(start) { return __replCall('test.loopTrace', {start:start === true}); },
    pluginState: function(instanceId) { return __replCall('test.pluginState', {instanceId:instanceId}); },
    instances: function() { return __replCall('test.instances', {}); }
};
// The player's catalog patch `patch` (its display path) for `pluginId`. Given `instance`
// (a track instance of the plugin; `created:true` when it is new), the patch goes into
// it: null when it went in directly (finishing after the call returns), else a state
// file to load into the instance. Without `instance`, a state file made without one.
repl.patchState = function(options) {
    return __replCall('patchState', {format:options.format, pluginId:options.pluginId, patch:options.patch,
        instance:options.instance, created:options.created === true});
};
// Fails on a patch ({format, pluginId, patch}) or effect preset ({format, pluginId,
// preset, value}) that repl.patchState or repl.effectState could not read.
repl.patchCheck = function(options) {
    return __replCall('patchCheck', {format:options.format, pluginId:options.pluginId, patch:options.patch,
        preset:options.preset, value:options.value});
};
// cat-plugin-player's shared catalog and Role / Kind classifier. Selection stays in JS.
repl.patchCatalog = function(options) {
    return __replCall('patchCatalog', {role:options.role, kind:options.kind});
};
// Reads the catalog again in the background; the next patchCatalog waits for it.
repl.reloadPatchCatalog = function() { return __replCall('patchCatalog.reload', {}); };
repl.catalogInstruments = function() { return __replCall('patchCatalog.instruments', {}); };
repl.midi = {
    controlChange: function(instanceId, controller, value) {
        return __replCall('midi.controlChange', {instanceId, controller, value});
    }
};
// The effect preset file `preset` (relative to the plugin's root) with the player's
// selector `value` inside it, for `pluginId`. `instance` and `created` are as in
// repl.patchState.
repl.effectState = function(options) {
    return __replCall('effectState', {format:options.format, pluginId:options.pluginId, preset:options.preset,
        value:options.value, instance:options.instance, created:options.created === true});
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
    close: function(id) { return __replCall('ui.close', {id:id}); },
    // A viewport of colored hit regions and an optional {title, checkbox, checked} header.
    // Poll returns {open, events} and, with a header, checked;
    // Primary capture emits down/move/up; right presses and vertical mouse wheel events
    // emit secondary and wheel for the hovered region.
    // Coordinates are normalized to the event's region.
    surface: {
        show: function(definition) { return __replCall('ui.surface.show', {definition}); },
        poll: function(id) { return __replCall('ui.surface.poll', {id}); },
        setTitle: function(id, title, padGainInstance) {
            return __replCall('ui.surface.setTitle', {id, title, padGainInstance});
        }
    },
    // A modal with an OK button; several wait in the order given.
    dialog: function(title, text) { return __replCall('ui.dialog', {title:title, text:text}); }
};

// Session-only UAPMD transport state; samples use the engine's current sample rate.
repl.timeline = {
    setLoop: function(startSamples, endSamples) {
        if (!(Number.isSafeInteger(startSamples) && startSamples >= 0 &&
              Number.isSafeInteger(endSamples) && endSamples > startSamples))
            throw new Error('Invalid loop range');
        return __replCall('timeline.setLoop', {startSamples:startSamples, endSamples:endSamples});
    },
    clearLoop: function() { return __replCall('timeline.clearLoop', {}); }
};

// Experimental session-only native source. Owns the selected track's MIDI input.
// set({track, steps:[[MIDI pitches]], bpm, beats, gate:(0,1], velocity:1..65535})
// `tracks:[...]` sends the pattern to up to eight distinct tracks instead of `track`.
// replaces the complete pattern at the next step; initial set starts at step 0.
// Rust evolve.dll uses the audio sample clock; gate:1 ties common notes.
// C++ only adapts UAPMD callbacks and UMP JR timestamps.
// Stop stops playback; clear also removes the pattern. Not part of project save.
repl.sequence = {
    pending: function() { return __replCall('sequence.pending', {}); },
    set: function(plan) {
        const result = __replCall('sequence.set', plan);
        repl.sequence.view = JSON.parse(JSON.stringify(plan));
        return result;
    },
    clear: function() {
        if (globalThis.evolveLoop) globalThis.evolveLoop.stop();
        const result = __replCall('sequence.clear', {});
        delete repl.sequence.view;
        return result;
    },
    choose: function(candidates, previous, maxLeap, pin = null) {
        return __replCall('sequence.choose', {candidates, previous, maxLeap, pin});
    }
};

// Notes of the clip `clipId` (a uapmd.timeline clip id), named by getClipNotes' noteId.
// cut returns how many notes it cut; paste puts the clipboard's first note at `tick`
// and returns the pasted notes' noteIds. Both save the clip.
repl.pianoRoll = {
    revoice: function(clipId, pitches) { return __replCall('pianoRoll.revoice', {clipId, pitches}); },
    cut: function(clipId, noteIds) {
        if (!Array.isArray(noteIds) || !noteIds.every(id => typeof id === 'string')) throw new Error('Invalid note ids');
        return __replCall('pianoRoll.cut', {clipId:clipId, noteIds:noteIds});
    },
    paste: function(clipId, tick) {
        if (!(Number.isInteger(tick) && tick >= 0)) throw new Error('Invalid paste tick');
        return __replCall('pianoRoll.paste', {clipId:clipId, tick:tick});
    }
};
