(() => {
    function prepare(slots, instruments = slots) {
        if (slots.length !== 2 || slots[0].track === slots[1].track)
            throw new Error('Expected two distinct evolution tracks');
        const patch = include('builtin/lib/patch.js');
        const choices = instruments.map(entry => ({...include('builtin/lib/plugins.js')
            .catalogEntry(entry.format, entry.plugin), ...entry}));
        const buffers = slots.map((entry, slot) => ({track:entry.track, slot, status:'empty', sound:null}));
        const call = (name, args={}) => __replCall('evolution.'+name, args);
        const job = (name, args) => __replCall('patchJob.'+name, args);
        let opened = false;
        let busy = false;
        let options;
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
        function poll() {
            if (!opened || busy) return;
            busy=true;
            try {
                const states=call('status');
                for (const buffer of buffers) {
                    const state=states[buffer.slot];
                    buffer.instanceId=state.instanceId;
                    if (buffer.loading && !state.loading) {
                        job('release',{jobId:buffer.preparation});
                        delete buffer.preparation;
                        delete buffer.loading;
                        if (state.error) console.log(state.error);
                    }
                    if (buffer.preparation !== undefined && !buffer.loading) {
                        const progress=job('get',{jobId:buffer.preparation});
                        if (progress.state==='pending') continue;
                        if (progress.state==='failed') {
                            console.log(progress.error);
                            job('release',{jobId:buffer.preparation});
                            delete buffer.preparation;
                        } else {
                            call('load',{slot:buffer.slot,format:buffer.sound.format,
                                pluginId:buffer.sound.pluginId,file:progress.file});
                            buffer.loading=true;
                            continue;
                        }
                    }
                    if (state.state===0 && buffer.preparation===undefined && buffer.status!=='failed') request(buffer);
                    else if (state.state===2) buffer.status='ready';
                    else if (state.state===3) { buffer.status='active'; buffer.remaining=8; }
                }
            } finally { busy=false; }
        }
        function stop() {
            repl.tasks.cancel('evolve-crossfade');
            if (!opened) return;
            const deadline=Date.now()+120000;
            while (call('status').some(s=>s.loading)) {
                if (Date.now()>=deadline) throw new Error('Evolution load did not finish');
                repl.processEvents();
            }
            call('close');
            opened=false;
            for (const buffer of buffers) {
                if (buffer.preparation!==undefined) job('release',{jobId:buffer.preparation});
                delete buffer.preparation;
                delete buffer.loading;
                buffer.status='empty';
            }
        }
        return {
            buffers, poll, stop, dispose:stop,
            start(sounds, chordMs, {holdMs,fadeMs,everyMs}) {
                stop();
                options=sounds;
                call('open',{tracks:buffers.map(b=>b.track),holdMs,fadeMs});
                opened=true;
                buffers.forEach(b=>{b.remaining=8;b.status='empty';});
                const deadline=Date.now()+120000;
                while (call('status')[0].state!==2) {
                    poll();
                    if (Date.now()>=deadline || buffers[0].status==='failed') {
                        stop(); throw new Error('Could not prepare evolution A');
                    }
                    repl.processEvents();
                }
                repl.tasks.every('evolve-crossfade',everyMs,poll);
            }
        };
    }
    return {prepare};
})();
