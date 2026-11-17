// @text timeline_comment_0
// @text timeline_comment_1
(() => ({
    state: uapmd.timeline.getState(),
    sampleRate: uapmd.sequencer.getSampleRate(),
    scopeTarget: repl.scope.target(),
    sequence: repl.sequence.view || null,
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
