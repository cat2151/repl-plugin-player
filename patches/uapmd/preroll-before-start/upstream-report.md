# The song-start note-on is sent once per preroll block

Revision: `277aa85468795267d6cf64953af587ea18c7f653`. Not sent.

Put a plugin that reports latency on one track (so `maxRenderLeadInSamples() > 0`)
and a MIDI clip with a note at position 0 on any track, then start playback from 0.
Each track input receives the note-on at position 0 once per preroll block plus once
for the real block, with no note-off in between. Plugins hear the note restart several
times, so the first note sounds different from the same note in later loop passes.

`LatencyCompensationManagerImpl::schedulePrerollFromAudiblePosition` starts the render
position at `-alignToQuantum(maxRenderLeadInSamples())`. The quantum is the engine
buffer capacity, so with `RealtimeSequencer` (capacity 8192) one sample of lead gives
8192 samples of preroll, 8 blocks at 1024 frames. In
`TimelineFacadeImpl::processTracksAudio`, `if (renderStartSample < 0) renderStartSample = 0;`
then renders `[0, blockFrames)` for every preroll block.

Suggested fix: remove the clamp. `processAudioForRenderSegment` already handles a
negative start: `computeClipRenderWindow` returns no window for a block entirely before 0,
and a window starting at the right offset for a block that crosses 0. The track input is
zeroed for the whole block before it is written.

Regression, engine capacity 1024, 256-frame blocks at 48 kHz, one track with 32 samples
of latency and one with none, each with a note-on at 0 (note-ons received):
before 5 (no latency) / 4 (latency 32); after 1 / 1.
