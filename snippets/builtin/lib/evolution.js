(() => {
    function start({mode, track, pool, tracks = pool ? pool.buffers.map(b=>b.track) : [track], crossfade, sounds, progression, inversions, octaves, maxLeap, range, bpm, beats, velocity, everyMs}) {
        if (mode !== 'sequence') throw new Error('Expected sequence');
        if (tracks.length < 1 || tracks.length > 2 || new Set(tracks).size !== tracks.length)
            throw new Error('Expected distinct evolution tracks');
        tracks.forEach(target => include('builtin/lib/tracks.js').info(target));
        const previous = globalThis.evolution;
        const fadeKey = JSON.stringify({sounds,crossfade,bpm,beats});
        const reuseFade = pool && previous && previous.pool===pool && previous.fadeKey===fadeKey
            && uapmd.timeline.getState().isPlaying;
        if (previous) previous.stop({keepFade:reuseFade});
        delete globalThis.evolution;
        const voice = include('builtin/lib/voicings.js').create(progression, {inversions, octaves, maxLeap, range});
        let pattern = voice.next();
        const publish = steps => repl.sequence.set({track:tracks[0],
            ...(tracks.length > 1 ? {tracks} : {}), steps, bpm, beats, gate:1, velocity});
        const soundKey = JSON.stringify({tracks, sounds});
        if (pool && !reuseFade) pool.start(sounds, beats*60000/bpm, crossfade);
        const patches = !pool && sounds && include('builtin/lib/patch.js');
        const selected = patches ? (previous && previous.soundKey === soundKey
            && uapmd.timeline.getState().isPlaying ? previous.sounds
            : patches.random({...sounds, tracks})) : [];
        const fade = reuseFade ? previous.fade : crossfade && (pool ? {stop:()=>pool.stop()}
            : include('builtin/lib/crossfade.js').start({tracks, ...crossfade,
            onSilent: patches ? target => {
                const next = patches.random({...sounds, tracks:[target], exclude:selected})[0];
                selected[tracks.indexOf(target)] = next;
            } : crossfade.onSilent}));
        try {
            publish(pattern.steps);
            uapmd.timeline.setTempo(bpm);
            repl.timeline.setLoop(0, Math.round(pattern.steps.length * beats * 60 / bpm * uapmd.sequencer.getSampleRate()));
            if (pool) uapmd.audio.setEngineEnabled(true);
            if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
        } catch (error) {
            if (fade) fade.stop();
            throw error;
        }
        const controller = {
            pattern,
            soundKey,
            sounds:selected,
            pool,
            fade,
            fadeKey,
            stop({keepFade=false}={}) { repl.tasks.cancel('evolve'); if (fade && !keepFade) fade.stop(); },
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
        globalThis.evolution = controller;
        repl.tasks.every('evolve', everyMs, controller.update);
        return controller;
    }
    return {start};
})();
