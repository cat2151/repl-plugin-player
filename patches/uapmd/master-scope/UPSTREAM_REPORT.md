# Final realtime output observer

An oscilloscope showing a crossfaded mix needs a tap after master processing,
soft clipping and output muting. Track event handlers cannot observe that output.
The local workaround wraps RealtimeSequencer's dispatcher callback, calls the
existing SequencerEngine processing, and observes the output on success before
returning. It leaves offline rendering and dependency checkout unchanged.

A final realtime output observer would remove the need for this source shim.
Tested against `4ec5c3b1c30130fdd3b7d4b375082273247e561f`. Not sent.
