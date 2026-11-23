(() => {
    const sequencer = uapmd.sequencer;
    const patch = include('builtin/lib/patch.js');
    const plugins = include('builtin/lib/plugins.js');

    function waitFor(ready) {
        const deadline = Date.now() + 10000;
        while (!ready()) {
            if (Date.now() >= deadline) throw new Error('Performance pad track change timed out');
            repl.processEvents();
        }
    }

    function mutation(job) {
        let result;
        waitFor(() => (result = uapmd.mutations.getJob(job.jobId)).state !== 'pending');
        uapmd.mutations.clearJob(job.jobId);
        if (result.state === 'failed') throw new Error(result.error);
        return result.result;
    }

    function regions({width, height, xy, colors}) {
        const gap = Math.min(width, height) / 64;
        const half = width / 2;
        const side = (Math.min(half, height) - 5 * gap) / 4;
        const gridSize = 4 * side + 3 * gap;
        const left = half + (half - gridSize) / 2;
        const top = (height - gridSize) / 2;
        return [{rect:[gap, gap, half - 2 * gap, height - 2 * gap], color:xy.color, cursor:true},
            ...colors.map((color, i) => ({color,
                rect:[left + (i % 4) * (side + gap), top + Math.floor(i / 4) * (side + gap), side, side]}))];
    }

    function instruments(capacity) {
        const key = p => p.format + '\n' + p.pluginId;
        const installed = new Set(Array.from({length:uapmd.catalog.getCount()}, (_, i) => key(uapmd.catalog.getPluginAt(i))));
        const groups = repl.catalogInstruments().filter(entry => installed.has(key(entry))).map(({patches, ...entry}) => {
            const candidates = [...new Set(patches)].map(name => ({...entry, patch:name}));
            return {candidates, remaining:[...candidates]};
        });
        if (!groups.length || groups.length > capacity)
            throw new Error(`Found ${groups.length} catalog instruments for ${capacity} pads`);
        return groups;
    }

    function reroll(pad) {
        const candidates = pad.candidates.filter(entry => entry.patch !== pad.patch);
        if (!candidates.length) return false;
        try {
            const chosen = patch.randomFrom(candidates, pad.trackIndex, pad.plugin, {load: entry => {
                plugins.loadPatch(pad.instance, false, entry, entry.patch);
            }});
            pad.patch = chosen.patch;
            return true;
        } catch (error) {
            console.log(error.message);
            return false;
        }
    }

    function play({xy, pads, instrumentCount, instrumentNote, chordNotes, select}) {
        let held;
        let cc;
        let lastPad;
        let chordMode = false;
        function release() {
            if (!held) return;
            for (const note of held.notes) sequencer.sendNoteOff(held.instance, note);
            held = undefined;
        }
        function noteOn(instance, notes, instrumentPad) {
            held = {instance, notes, instrumentPad};
            for (const note of notes) sequencer.sendNoteOn(instance, note);
        }
        function playInstrument(pad) {
            noteOn(pad.instance, chordMode ? chordNotes : [instrumentNote], pad);
        }
        function pointer({kind, region, x, y}) {
            if (kind === 'wheel') {
                if (region < 1 || region > instrumentCount) return;
                chordMode = !chordMode;
                const pad = held?.instrumentPad;
                if (pad) {
                    release();
                    playInstrument(pad);
                }
                return;
            }
            if (kind === 'secondary') {
                const pad = region === 0 ? lastPad : pads[region - 1];
                if (!pad?.candidates || pad.candidates.length < 2) return;
                if (held?.instance === pad.instance) release();
                if (reroll(pad) && pad === lastPad) select(pad);
                return;
            }
            if (kind === 'up') { release(); return; }
            if (region !== 0) {
                if (kind === 'down') {
                    release();
                    lastPad = pads[region - 1];
                    if (region <= instrumentCount) playInstrument(lastPad);
                    else noteOn(lastPad.instance, [lastPad.note]);
                    select(lastPad);
                }
                return;
            }
            if (!lastPad) return;
            const {instance} = lastPad;
            const noteIndex = Math.min(Math.floor(x * xy.notes.length), xy.notes.length - 1);
            const note = xy.notes[noteIndex];
            const value = Math.round((1 - y) * 127);
            const changed = kind === 'down' || held?.notes[0] !== note;
            if (changed) release();
            if (kind === 'down' || cc !== value) {
                repl.midi.controlChange(instance, xy.controller, value);
                cc = value;
            }
            if (changed) noteOn(instance, [note]);
        }
        return {pointer, release};
    }

    function open(options) {
        const {id, width, height, xy, drums, colors, instrumentNote, instrumentChord} = options;
        const progression = cmrt_chord.note_progression(instrumentChord);
        if (progression.length !== 1) throw new Error('Expected one instrument chord: ' + instrumentChord);
        const [chordNotes] = progression;
        const drumNotes = [...drums.notes, drums.percussion[Math.floor(Math.random() * drums.percussion.length)]];
        if (colors.length !== 16 || drumNotes.length >= colors.length || drumNotes.some(note => !Number.isInteger(note)))
            throw new Error('Expected 16 pad colors and valid drum note mappings');
        const capacity = colors.length - drumNotes.length;
        const groups = instruments(capacity);
        globalThis.performancePadSession?.();
        const trackIds = [];
        const scope = repl.scope.target();
        let player;
        let gainEnabled = false;
        let instances;
        function setGain(enabled) {
            repl.audio.setPadAutoGain(enabled ? instances : []);
            gainEnabled = enabled;
        }
        function addTrack() {
            const {trackId} = mutation(sequencer.addTrack());
            trackIds.push(trackId);
            const index = sequencer.getTrackInfos().findIndex(t => t.trackId === trackId);
            if (index < 0) throw new Error('Performance pad track was not added');
            sequencer.setTrackFreezePolicy(trackId, 'off');
            return index;
        }
        const owned = () => sequencer.getTrackInfos().filter(t => trackIds.includes(t.trackId));
        function close() {
            repl.tasks.cancel(id);
            repl.app.onExit(id, null);
            if (globalThis.performancePadSession === close) delete globalThis.performancePadSession;
            repl.ui.close(id);
            if (gainEnabled) setGain(false);
            player?.release();
            const nodes = owned().flatMap(t => t.nodes).filter(n => n.instanceId >= 0);
            if (nodes.some(n => n.instanceId === repl.scope.target())) {
                const ids = sequencer.getInstanceIds();
                repl.scope.setTarget(scope < 0 || ids.includes(scope) ? scope : -1);
            }
            for (const node of nodes) uapmd.instance(node.instanceId).dispose();
            waitFor(() => owned().every(t => t.nodes.every(n => n.instanceId < 0)));
            for (const track of owned().reverse()) mutation(sequencer.removeTrack(track.trackId));
        }
        repl.ui.surface.show({id, width, height, regions:regions(options),
            header:{checkbox:'auto gain', checked:true, padGainMeter:true}});
        globalThis.performancePadSession = close;
        try {
            const drumInstance = patch.use({...drums, track:addTrack()}).instanceId;
            const pads = [];
            for (let i = 0; i < capacity; ++i) {
                const available = groups.filter(group => group.remaining.length > 0);
                const group = groups[i] ?? available[Math.floor(Math.random() * available.length)];
                if (!group) throw new Error('Not enough playable catalog patches for the pads');
                const chosen = patch.randomFrom(group.remaining, addTrack(), group.candidates[0]?.plugin || 'instrument');
                pads.push({...chosen, candidates:group.candidates, instance:chosen.instanceId});
            }
            pads.push(...drumNotes.map(note => ({...drums, instance:drumInstance, note})));
            instances = [...new Set(pads.map(pad => pad.instance))];
            player = play({xy, pads, instrumentCount:capacity, instrumentNote, chordNotes, select:({plugin, patch, instance}) => {
                const parts = patch.split(/[\\/]/);
                const file = parts.pop();
                const name = /^main\.sfz$/i.test(file) && parts.length
                    ? parts.pop() : file.replace(/\.(fxp|sfz|json|vstpreset|clap-preset)$/i, '');
                repl.ui.surface.setTitle(id, plugin + ' / ' + name, instance);
            }});
            setGain(true);
            uapmd.audio.setEngineEnabled(true);
            repl.app.onExit(id, close);
            repl.tasks.every(id, 10, () => {
                try {
                    const frame = repl.ui.surface.poll(id);
                    if (!frame.open) { close(); return; }
                    if (frame.checked !== gainEnabled) setGain(frame.checked);
                    for (const event of frame.events) player.pointer(event);
                } catch (error) {
                    if (globalThis.performancePadSession === close) close();
                    throw error;
                }
            });
        } catch (error) {
            close();
            throw error;
        }
    }

    return {open};
})();
