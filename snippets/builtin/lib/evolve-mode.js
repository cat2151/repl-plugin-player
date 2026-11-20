(() => {
    function wait(ready) {
        const deadline = Date.now() + 120000;
        while (!ready()) {
            if (Date.now() >= deadline) throw new Error('evolve cleanup timed out');
            repl.processEvents();
        }
    }
    function mutation(job) {
        wait(() => uapmd.mutations.getJob(job.jobId).state !== 'pending');
        const result = uapmd.mutations.getJob(job.jobId);
        if (result.state === 'failed') throw new Error(result.error);
    }
    function end() {
        const mode = globalThis.evolveMode;
        if (!mode) return;
        repl.tasks.cancel('evolve');
        repl.tasks.cancel('evolve-crossfade');
        include('builtin/lib/tutorial.js').stop();
        if (globalThis.evolveLoop) globalThis.evolveLoop.stop();
        delete globalThis.evolveLoop;
        repl.sequence.clear();
        if (globalThis.evolvePool) globalThis.evolvePool.dispose();
        // UAPMD's track removal fails when it also has to remove live plugins.
        for (const track of uapmd.sequencer.getTrackInfos().filter(t => mode.trackIds.includes(t.trackId)))
            for (const node of track.nodes.filter(n => n.instanceId >= 0)) uapmd.instance(node.instanceId).dispose();
        wait(() => uapmd.sequencer.getTrackInfos().filter(t => mode.trackIds.includes(t.trackId))
            .every(t => t.nodes.every(n => n.instanceId < 0)));
        for (const trackId of [...mode.trackIds].reverse()) {
            if (uapmd.sequencer.getTrackInfos().some(t => t.trackId === trackId))
                mutation(uapmd.sequencer.removeTrack(trackId));
        }
        const master = uapmd.timeline.getMasterTrackId();
        for (const clip of uapmd.timeline.getClips(master)) {
            if (!mode.masterClips.includes(clip.clipId))
                if (!uapmd.timeline.removeClip(clip.clipId)) throw new Error('Could not remove evolve tempo clip');
        }
        const nodes = uapmd.sequencer.getTrackInfos().flatMap(t => t.nodes);
        for (const node of mode.bypass) {
            if (nodes.some(n => n.instanceId === node.instanceId))
                uapmd.sequencer.setPluginBypassed(node.instanceId, node.bypassed);
        }
        for (const saved of mode.volumes) {
            const index=uapmd.sequencer.getTrackInfos().findIndex(t=>t.trackId===saved.trackId);
            if (index>=0) repl.track.setVolume(index,saved.level);
        }
        repl.timeline.clearLoop();
        if (mode.state.loopEnabled) repl.timeline.setLoop(mode.state.loopStartSamples, mode.state.loopEndSamples);
        globalThis.loopParts = mode.loopParts;
        uapmd.timeline.setTempo(mode.tempo);
        repl.scope.setTarget(mode.scope);
        delete globalThis.evolvePool;
        delete globalThis.evolveMode;
    }
    function enter(slots) {
        const key = JSON.stringify(slots);
        if (globalThis.evolveMode) {
            if (globalThis.evolveMode.slots===key) return globalThis.evolvePool;
            end();
        }
        const before = uapmd.sequencer.getTrackInfos();
        const mode = {
            slots:key, trackIds:[], scope:repl.scope.target(), loopParts:globalThis.loopParts,
            tempo:uapmd.timeline.getState().tempo, state:uapmd.timeline.getState(),
            volumes:before.map((t,index)=>({trackId:t.trackId,level:repl.track.volume(index)})),
            masterClips:uapmd.timeline.getClips(uapmd.timeline.getMasterTrackId()).map(c => c.clipId),
            bypass:before.flatMap(t => t.nodes.filter(n => n.instanceId >= 0).map(n =>
                ({instanceId:n.instanceId, bypassed:uapmd.sequencer.isPluginBypassed(n.instanceId)})))
        };
        globalThis.evolveMode = mode;
        try {
            include('builtin/lib/tutorial.js').stop();
            delete globalThis.loopParts;
            for (const node of mode.bypass) uapmd.sequencer.setPluginBypassed(node.instanceId, true);
            // Frozen/audio tracks can sound without passing through the plugins.
            before.forEach((track,index)=>repl.track.setVolume(index,0));
            for (let index=0; index<2; ++index) {
                const ids = uapmd.sequencer.getTrackInfos().map(t => t.trackId);
                mutation(uapmd.sequencer.addTrack());
                mode.trackIds.push(...uapmd.sequencer.getTrackInfos().filter(t => !ids.includes(t.trackId)).map(t => t.trackId));
            }
            globalThis.evolvePool = include('builtin/lib/evolve-pool.js').prepare(
                [0,1].map(index => ({track:before.length + index})), slots);
            for (const trackId of mode.trackIds) uapmd.sequencer.setTrackFreezePolicy(trackId, 'off');
            repl.scope.setTarget(-2); // Final stereo master mix.
            return globalThis.evolvePool;
        } catch (error) {
            end();
            throw error;
        }
    }
    return {enter, end};
})();
