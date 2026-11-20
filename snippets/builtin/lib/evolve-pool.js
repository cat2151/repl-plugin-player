// The slot groups of evolve: a request returns at once and its outcome is read by
// the next tick, so nothing here waits for a load. Each group is two tracks that
// crossfade; its slots follow the previous groups' slots (as evolve numbers them).
// The returned pool is the first group; `groups` holds them all.
(() => {
    function prepare(layout, instruments) {
        let first = 0;
        const groups = layout.map(({tracks, input}) => {
            if (tracks.length !== 2 || tracks[0] === tracks[1])
                throw new Error('Expected two distinct evolve tracks');
            const buffers = tracks.map((track, i) => ({track, slot:first + i, status:'empty', sound:null}));
            first += tracks.length;
            // `pick(buffer, load, exclude)` returns the chosen sound, or nothing to wait.
            return {tracks, input, buffers, pick:null};
        });
        const all = groups.flatMap(g => g.buffers);
        const patch = include('builtin/lib/patch.js');
        const choices = instruments.map(entry => ({...include('builtin/lib/plugins.js')
            .catalogEntry(entry.format, entry.plugin), ...entry}));
        const call = (name, args={}) => __replCall('evolve.'+name, args);
        const job = (name, args) => __replCall('patchJob.'+name, args);
        let opened;
        function release(buffer) {
            if (buffer.preparation !== undefined) job('release', {jobId:buffer.preparation});
            delete buffer.preparation;
        }
        function request(group, buffer) {
            if (!group.pick) return;
            const load = selected => { buffer.preparation=job('start',selected); return {}; };
            try {
                const exclude=group.buffers.map(b=>b.sound).filter(Boolean);
                if ((buffer.remaining ?? 8) <= 0) { buffer.status='failed'; return; }
                const sound=group.pick(buffer, load, exclude);
                if (!sound) return;
                buffer.remaining=(buffer.remaining ?? 8)-1;
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
                const buffer=all.find(b=>b.request===outcome.requestId);
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
            for (const group of groups) for (const buffer of group.buffers) {
                const state=states[buffer.slot];
                buffer.instanceId=state.instanceId;
                if (buffer.preparation!==undefined) submit(buffer);
                else if (state.state===0 && buffer.request===undefined && buffer.status!=='failed') request(group, buffer);
                else if (state.state===2) buffer.status='ready';
                else if (state.state===3) { buffer.status='active'; buffer.remaining=8; }
            }
        }
        // Keeps the sounding instances; only the requests of this cycle are dropped.
        function stop() {
            repl.tasks.cancel('evolve-crossfade');
            if (!opened) return;
            call('bumpGeneration');
            for (const buffer of all) {
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
            all.forEach(b=>{b.status='empty'; b.sound=null;});
        }
        const [pads] = groups;
        return {
            buffers:pads.buffers, groups, poll, stop, dispose,
            start(sounds, chordMs, {holdMs,fadeMs,everyMs}) {
                stop();
                if (opened && (opened.holdMs!==holdMs || opened.fadeMs!==fadeMs)) dispose();
                pads.pick=(buffer, load, exclude)=>patch.random({...sounds, tracks:[buffer.track],
                    instruments:choices, exclude, load})[0];
                if (!opened) {
                    call('open',{groups:groups.map(g=>({tracks:g.tracks,input:g.input})),holdMs,fadeMs});
                    opened={holdMs,fadeMs};
                }
                all.forEach(b=>{b.remaining=8; if (b.status==='failed') b.status='empty';});
                poll();
                repl.tasks.every('evolve-crossfade',everyMs,poll);
            }
        };
    }
    return {prepare};
})();
