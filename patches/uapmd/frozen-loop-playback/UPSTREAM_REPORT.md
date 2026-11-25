# Frozen tracks read past the loop end in the callback that wraps

Revision: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.
Status: 未報告. This file is a draft, not an external submission.

`FrozenTrackManager::processAudio` maps the callback's first position into the cache
and reads all frames contiguously. The transport now wraps at the loop end, but a
callback that crosses it still reads cached audio after the loop end for its remaining
frames, while live clips continue at the loop start. Split the read at the loop end,
as `TimelineFacadeImpl::processTracksAudio` does. Offline rendering should continue
to ignore realtime loop state. This is found by reading the source.

Regression coverage includes exact sample positions for zero/nonzero loop starts,
nonaligned callback boundaries, multiple wraps per block, disabled/invalid loops,
and continuous offline positions, plus real-application loop playback.
