// @text timeline_comment_0
// @text timeline_comment_1
(() => ({
    state: uapmd.timeline.getState(),
    sampleRate: uapmd.sequencer.getSampleRate(),
    scopeTarget: repl.scope.target(),
    sequence: repl.sequence.view || null,
    evolve: (() => {
        const pool = globalThis.evolvePool;
        if (!pool) return [];
        let slots;
        // Throws until the pool's first start opens evolve.
        try { slots = __replCall('evolve.status', {}); } catch (error) { return []; }
        // The pads and the scatter: each opened slot of each group with its fade.
        return pool.groups.flatMap(g => g.buffers).filter(b => slots[b.slot]).map(b => ({track:b.track, gain:slots[b.slot].gain,
            state:slots[b.slot].state, plugin:b.sound ? b.sound.plugin : null, patch:b.sound ? b.sound.patch : null}));
    })(),
    tracks: uapmd.sequencer.getTrackInfos().map((track, trackIndex) => ({
        trackIndex,
        trackId: track.trackId,
        nodes: track.nodes,
        graph: uapmd.timeline.getTrackGraphConnections(track.trackId),
        clips: uapmd.timeline.getClips(track.trackId).map(clip => ({
            ...clip,
            midi: clip.clipType === 'midi'
                ? uapmd.timeline.getClipUmpEvents(clip.clipId)
                : null
        }))
    }))
}))()
