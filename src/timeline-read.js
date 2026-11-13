// @text timeline_comment_0
// @text timeline_comment_1
(() => ({
    state: uapmd.timeline.getState(),
    sampleRate: uapmd.sequencer.getSampleRate(),
    tracks: uapmd.sequencer.getTrackInfos().map((track, trackIndex) => ({
        trackIndex,
        nodes: track.nodes,
        clips: uapmd.timeline.getClips(track.trackId).map(clip => ({
            ...clip,
            midi: clip.clipType === 'midi'
                ? uapmd.timeline.getClipUmpEvents(clip.clipId)
                : null
        }))
    }))
}))()
