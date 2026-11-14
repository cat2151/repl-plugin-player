# Loop playback drops Note Off exactly at loopEnd

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`

Create a MIDI clip whose last Note Off is exactly at the transport loop end,
then enable looping and play several passes. The final Note On is delivered
each pass but its Note Off never reaches the plugin. Notes accumulate or sustain.

`TimelineFacadeImpl::processTracksAudio` renders end-exclusive segments and wraps
at loopEnd. `MidiClipSourceNode::processEvents` excludes events at windowEnd,
so the endpoint release is skipped on every pass.

The local workaround admits only Note Off (including velocity-zero Note On,
MIDI 1.0 and 2.0) at that endpoint, clamped to the outgoing segment's last sample.
Ordinary Note On and other messages at the endpoint remain excluded, even when
they precede Note Off in source order. Events beyond loopEnd remain excluded.
Normal non-loop playback retains its end-exclusive windows.

Regression: plugin-input trace of a four-note looping clip at 120 BPM,
plus direct MidiClipSourceNode tests for filtering and block offsets.
