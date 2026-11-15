// %% @text name_freeze_track
// Rendering waits for stopped playback and a quiet tail; show-track-freeze reports readiness.
{
    const listenTrack = 0;
    const trackId = include('builtin/lib/tracks.js').idOf(listenTrack);
    uapmd.sequencer.setTrackFreezePolicy(trackId, 'on');
    include('builtin/lib/freeze.js').describe(trackId);
}
