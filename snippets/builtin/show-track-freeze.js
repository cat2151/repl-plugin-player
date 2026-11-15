// %% @text name_show_track_freeze
{
    const listenTrack = 0;
    const trackId = include('builtin/lib/tracks.js').idOf(listenTrack);
    include('builtin/lib/freeze.js').describe(trackId);
}
