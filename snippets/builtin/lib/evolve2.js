// The same pool interface as evolution-pool.js, over evolve2: a request returns at
// once and its outcome is read by the next tick, so nothing here waits for a load.
(() => {
    function prepare(slots, instruments = slots) {
        if (slots.length !== 2 || slots[0].track === slots[1].track)
            throw new Error('Expected two distinct evolution tracks');
        const patch = include('builtin/lib/patch.js');
        const choices = instruments.map(entry => ({...include('builtin/lib/plugins.js')
            .catalogEntry(entry.format, entry.plugin), ...entry}));
        const buffers = slots.map((entry, slot) => ({track:entry.track, slot, status:'empty', sound:null}));
        const call = (name, args={}) => __replCall('evolve2.'+name, args);
        const job = (name, args) => __replCall('patchJob.'+name, args);
        let opened;
        let options;
        function release(buffer) {
            if (buffer.preparation !== undefined) job('release', {jobId:buffer.preparation});
            delete buffer.preparation;
        }
        function request(buffer) {
            buffer.remaining = (buffer.remaining ?? 8)-1;
            if (buffer.remaining < 0) { buffer.status='failed'; return; }
            try {
                const [sound] = patch.random({...options, tracks:[buffer.track], instruments:choices,
                    exclude:buffers.map(b=>b.sound).filter(Boolean), load:selected=>{
                        buffer.preparation=job('start',selected);
                        return {};
                    }});
                buffer.sound=sound;
                buffer.status='loading';
            } catch (error) { buffer.status='failed'; console.log(error.message); }
        }
        function submit(buffer) {
            const progress=job('get',{jobId:buffer.preparation});
            if (progress.state==='pending') return;
            if (progress.state==='failed') console.log(progress.error);
            else buffer.request=call('submit',{slot:buffer.slot,format:buffer.sound.format,
                pluginId:buffer.sound.pluginId,file:progress.file});
            // The state bytes were read by the submit.
            release(buffer);
        }
        function settle() {
            for (const outcome of call('poll')) {
                const buffer=buffers.find(b=>b.request===outcome.requestId);
                // Requests made before the last bumpGeneration belong to nobody.
                if (!buffer) continue;
                delete buffer.request;
                if (!outcome.stale && outcome.error) console.log(outcome.error);
            }
        }
        function poll() {
            if (!opened) return;
            settle();
            const states=call('status');
            for (const buffer of buffers) {
                const state=states[buffer.slot];
                buffer.instanceId=state.instanceId;
                if (buffer.preparation!==undefined) submit(buffer);
                else if (state.state===0 && buffer.request===undefined && buffer.status!=='failed') request(buffer);
                else if (state.state===2) buffer.status='ready';
                else if (state.state===3) { buffer.status='active'; buffer.remaining=8; }
            }
        }
        // Keeps the sounding instances; only the requests of this cycle are dropped.
        function stop() {
            repl.tasks.cancel('evolve-crossfade');
            if (!opened) return;
            call('bumpGeneration');
            for (const buffer of buffers) {
                release(buffer);
                delete buffer.request;
                if (buffer.status!=='active') buffer.status='empty';
            }
        }
        function dispose() {
            stop();
            if (!opened) return;
            call('close');
            opened=undefined;
            buffers.forEach(b=>{b.status='empty'; b.sound=null;});
        }
        return {
            buffers, poll, stop, dispose,
            start(sounds, chordMs, {holdMs,fadeMs,everyMs}) {
                stop();
                if (opened && (opened.holdMs!==holdMs || opened.fadeMs!==fadeMs)) dispose();
                options=sounds;
                if (!opened) {
                    call('open',{tracks:buffers.map(b=>b.track),holdMs,fadeMs});
                    opened={holdMs,fadeMs};
                }
                buffers.forEach(b=>{b.remaining=8; if (b.status==='failed') b.status='empty';});
                poll();
                repl.tasks.every('evolve-crossfade',everyMs,poll);
            }
        };
    }
    return {prepare};
})();
