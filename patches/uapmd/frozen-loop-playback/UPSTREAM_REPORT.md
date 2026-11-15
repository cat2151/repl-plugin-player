# Frozen tracks become silent after the first loop pass

Tested revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告. This file is a draft, not an external submission.

Create a MIDI clip, enable its freeze policy, wait for `frozen`, enable a one-second
transport loop, and play for five seconds. Ordinary clips wrap, but frozen playback
plays its recorded content once and then goes silent. A test-owned Surge clip
produced pass peaks `[0.275, 2.1e-7, 5.2e-17, 0, 0]` before the repair.

`FrozenTrackManager::processAudio` uses the callback context's increasing transport
sample position as a cache offset. That position is not wrapped to the timeline
loop. Frozen audio reads must map it into the loop range and split reads crossing
the endpoint, with matching loader positions. Offline rendering should continue
to ignore realtime loop state. The local repair leaves the nonblocking disk loader
and cache serialization unchanged.

Regression coverage includes exact sample positions for zero/nonzero loop starts,
nonaligned callback boundaries, multiple wraps per block, disabled/invalid loops,
and continuous offline positions, plus real-application loop playback.
