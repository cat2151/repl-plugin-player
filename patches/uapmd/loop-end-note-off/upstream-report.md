# Notes hang when the loop end falls on an audio block boundary

Revision: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`. Not sent.

Play a MIDI clip that outlasts the loop region, with the note still held at the
loop end and the loop end exactly on an audio block boundary (for example
`loopRegion({true, 0, 2048})` with 256-frame blocks). The note receives a Note On
every pass and never a Note Off. With the loop end inside a block, the Note Off is sent.

`TimelineFacadeImpl::processTracksAudio` sets `wrapsAtLoopEnd` only when
`segmentStartSample + remainingFrames > loop.endSamples`. At equality the segment
is not treated as leaving the loop, so `sendNoteOffsAtEnd` is false. The next block
seeks to the loop start, and `MidiClipSourceNode::seek` clears the active notes.

Suggested fix: compare with `>=`. The rendering loop then ends with no frames remaining.

Regression, 64 blocks of 256 frames at 48 kHz (Note On / Note Off counts):
loop 0..2048: 8/0 before, 8/8 after; loop 512..2048: 11/0 before, 11/10 after;
loop 0..2148: 8/7 both.
