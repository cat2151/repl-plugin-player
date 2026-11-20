// Two tracks beside the evolve pads: chord tones of the playing pattern at random
// times, on a catalog sound that changes at random intervals. Each chord has its
// own clip on each track; once the playhead has left a chord, its clips are drawn
// again for the next pass. The timeline plays the clips, so JS keeps no clock.
(() => {
    const patch = include('builtin/lib/patch.js');
    const plugins = include('builtin/lib/plugins.js');
    const between = (min, max) => min + Math.random() * (max - min);

    function trackIds(tracks) {
        const infos = uapmd.sequencer.getTrackInfos();
        return tracks.map(track => {
            if (!infos[track]) throw new Error('evolve scatter track is missing');
            return infos[track].trackId;
        });
    }

    function addNote(clipId, tick, pitch, velocity) {
        if (!uapmd.timeline.addUmpEvent(clipId, tick.on, [0x40900000 + pitch * 0x100, velocity * 0x10000])
            || !uapmd.timeline.addUmpEvent(clipId, tick.off, [0x40800000 + pitch * 0x100, 0]))
            throw new Error('Could not add an evolve scatter note');
    }

    function removeClip(clipId) {
        if (!uapmd.timeline.removeClip(clipId)) throw new Error('Could not remove clip');
    }

    // On the grid, the chord sounds a tone of its pitch classes with `chance`. Both
    // tracks get the same notes: the sounding slot and the one fading in play them.
    function drawChord(ids, loop, index, {range, grid, chance, length, velocity}) {
        const seconds = loop.beats * 60 / loop.bpm;
        const clipIds = ids.map(id => uapmd.timeline.createEmptyMidiClip(id,
            Math.round(index * seconds * uapmd.sequencer.getSampleRate()), 480, loop.bpm));
        if (!clipIds.every(Boolean)) throw new Error('Could not create an evolve scatter clip');
        const perBeat = uapmd.timeline.getClipUmpEvents(clipIds[0]).tickResolution;
        const classes = new Set(loop.pattern.steps[index].map(n => n % 12));
        const tones = [];
        for (let pitch = range.min; pitch <= range.max; ++pitch) if (classes.has(pitch % 12)) tones.push(pitch);
        if (!tones.length) throw new Error('No chord tone in the evolve scatter range');
        const end = loop.beats * perBeat;
        for (let beat = 0; beat < loop.beats; beat += grid) {
            if (Math.random() >= chance) continue;
            const on = Math.round(beat * perBeat);
            // A note-off past the chord would be lost when its clip is drawn again.
            const off = Math.min(end - 1, on + Math.round(length * perBeat));
            const pitch = tones[Math.floor(Math.random() * tones.length)];
            const level = Math.round(between(velocity.min, velocity.max));
            for (const clipId of clipIds) addNote(clipId, {on, off}, pitch, level);
        }
        return clipIds;
    }

    // The chord under the playhead; the loop starts at 0 (evolve-loop.js).
    function chordAt(loop) {
        const seconds = loop.beats * 60 / loop.bpm;
        const at = uapmd.timeline.getState().playheadSamples / uapmd.sequencer.getSampleRate();
        return Math.floor(at / seconds) % loop.pattern.steps.length;
    }

    // The sound changes through the `timeline` group of evolve (evolve-pool.js):
    // a slot is requested when it is empty and the random interval has passed.
    function start({loop, instruments, sounds, notes, changeMs, everyMs}) {
        const pool = globalThis.evolvePool;
        if (!globalThis.evolveMode || !pool || !loop) throw new Error('Start evolve before its scatter track');
        if (globalThis.evolveScatter) globalThis.evolveScatter.stop();
        const group = pool.groups.find(g => g.input === 'timeline');
        const ids = trackIds(group.tracks);
        for (const id of ids) for (const clip of uapmd.timeline.getClips(id)) removeClip(clip.clipId);
        const clips = loop.pattern.steps.map((_, index) => drawChord(ids, loop, index, notes));
        const choices = instruments.map(entry => ({...plugins.catalogEntry(entry.format, entry.plugin), ...entry}));
        const controller = {buffers:group.buffers, clips, nextAt:0, stop() {
            repl.tasks.cancel('evolve-scatter');
            group.pick = null;
        }};
        let playing = chordAt(loop);
        function redraw() {
            if (!uapmd.timeline.getState().isPlaying) return;
            const now = chordAt(loop);
            // Every chord the playhead left since the last tick.
            for (; playing !== now; playing = (playing + 1) % clips.length) {
                clips[playing].forEach(removeClip);
                clips[playing] = drawChord(ids, loop, playing, notes);
            }
        }
        group.pick = (buffer, load, exclude) => {
            if (Date.now() < controller.nextAt) return undefined;
            controller.nextAt = Date.now() + between(changeMs.min, changeMs.max);
            const failures = [];
            const kinds = sounds.kinds.slice().sort(() => Math.random() - 0.5);
            for (const kind of kinds) {
                try {
                    return patch.random({tracks:[buffer.track], role:sounds.role, kind,
                        instruments:choices, exclude, load})[0];
                } catch (error) { failures.push(kind + ': ' + error.message); }
            }
            // Tried again once the next interval has passed.
            console.log(failures.join('\n'));
            return undefined;
        };
        globalThis.evolveScatter = controller;
        redraw();
        repl.tasks.every('evolve-scatter', everyMs, redraw);
        return controller;
    }

    return {start};
})();
