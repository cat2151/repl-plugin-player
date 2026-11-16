(() => {
    function mix(elapsed, holdMs, fadeMs) {
        const leg = holdMs + fadeMs;
        const phase = elapsed % leg;
        const blend = Math.max(0, (phase - holdMs) / fadeMs);
        return Math.floor(elapsed / leg) % 2 === 0 ? blend : 1 - blend;
    }
    function start({tracks, holdMs, fadeMs, everyMs, onSilent}) {
        if (tracks.length !== 2 || tracks[0] === tracks[1] ||
            ![holdMs, fadeMs, everyMs].every(n => Number.isFinite(n) && n > 0) || everyMs < 10)
            throw new Error('Invalid crossfade settings');
        const levels = tracks.map(track => repl.track.volume(track));
        let elapsed = 0;
        let previous = Date.now();
        let wasPlaying = uapmd.timeline.getState().isPlaying;
        let stopped = false;
        let busy = false;
        const apply = () => {
            const blend = mix(elapsed, holdMs, fadeMs);
            repl.track.setVolume(tracks[0], levels[0] * (1 - blend));
            repl.track.setVolume(tracks[1], levels[1] * blend);
        };
        apply();
        repl.tasks.every('evolve-crossfade', everyMs, () => {
            if (busy || stopped) return;
            const now = Date.now();
            const playing = uapmd.timeline.getState().isPlaying;
            const leg = holdMs + fadeMs;
            const boundary = (Math.floor(elapsed / leg) + 1) * leg;
            if (playing && wasPlaying) elapsed = Math.min(boundary, elapsed + Math.max(0, now - previous));
            previous = now;
            wasPlaying = playing;
            apply();
            if (elapsed === boundary && onSilent) {
                // Pin the endpoint even if JS missed the entire hold. Loading may
                // process events: neither re-entry nor load time may start a fade.
                busy = true;
                try { onSilent(tracks[Math.floor(elapsed / leg) % 2 === 1 ? 0 : 1]); }
                catch (error) {
                    stopped = true;
                    repl.tasks.cancel('evolve-crossfade');
                    throw error;
                } finally {
                    previous = Date.now();
                    busy = false;
                }
            }
        });
        let restored = false;
        return {stop() {
            if (restored) return;
            restored = true;
            stopped = true;
            repl.tasks.cancel('evolve-crossfade');
            tracks.forEach((track, i) => repl.track.setVolume(track, levels[i]));
        }};
    }
    return {mix, start};
})();
