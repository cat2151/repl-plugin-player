// %% @text name_unfreeze_track
{
    const listenTrack = 0;
    const trackId = include('builtin/lib/tracks.js').idOf(listenTrack);
    uapmd.sequencer.setTrackFreezePolicy(trackId, 'off');
}
