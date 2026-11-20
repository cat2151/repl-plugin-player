(() => {
    function start({mode, pool, crossfade, sounds, progression, inversions, octaves, maxLeap, range, bpm, beats, velocity, everyMs}) {
        if (mode !== 'sequence') throw new Error('Expected sequence');
        const tracks = pool.buffers.map(b=>b.track);
        tracks.forEach(target => include('builtin/lib/tracks.js').info(target));
        const previous = globalThis.evolveLoop;
        const fadeKey = JSON.stringify({sounds,crossfade,bpm,beats});
        const reuseFade = previous && previous.pool===pool && previous.fadeKey===fadeKey
            && uapmd.timeline.getState().isPlaying;
        if (previous) previous.stop({keepFade:reuseFade});
        delete globalThis.evolveLoop;
        const voice = include('builtin/lib/voicings.js').create(progression, {inversions, octaves, maxLeap, range});
        let pattern = voice.next();
        const publish = steps => repl.sequence.set({track:tracks[0], tracks, steps, bpm, beats, gate:1, velocity});
        if (!reuseFade) pool.start(sounds, beats*60000/bpm, crossfade);
        try {
            publish(pattern.steps);
            uapmd.timeline.setTempo(bpm);
            repl.timeline.setLoop(0, Math.round(pattern.steps.length * beats * 60 / bpm * uapmd.sequencer.getSampleRate()));
            uapmd.audio.setEngineEnabled(true);
            if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
        } catch (error) {
            pool.stop();
            throw error;
        }
        const controller = {
            pattern,
            pool,
            bpm,
            beats,
            fadeKey,
            stop({keepFade=false}={}) { repl.tasks.cancel('evolve'); if (!keepFade) pool.stop(); },
            update() {
                const state = uapmd.timeline.getState();
                if (!state.isPlaying) return;
                if (repl.sequence.pending()) return;
                const next = voice.next();
                publish(next.steps);
                pattern = next;
                controller.pattern = next;
            }
        };
        globalThis.evolveLoop = controller;
        repl.tasks.every('evolve', everyMs, controller.update);
        return controller;
    }
    return {start};
})();
