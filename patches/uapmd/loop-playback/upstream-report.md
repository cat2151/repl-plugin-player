# Proposal: expose safe loop configuration and prepared boundary publication

Observed revision: a4a96938eb31ddcb70209d62caf0dcc6ea9d3abe.

TimelineFacadeAudio already wraps render positions and splits blocks at loop end,
but the public scripting API does not expose safe loop configuration or prepared
MIDI publication at a common future render boundary. Polling the audible playhead
and replacing clips misses render lookahead and per-track/master latency.

The app generates a local copy with callback preparation, raw-to-phase mapping,
segment splitting and per-track render-segment MIDI input. Prepared immutable data remains app-owned;
the audio path only selects snapshots and appends bounded events. Transport Stop
cancels reservations through PlaybackEngineExtension. Targeted note termination
at wrap handles end-exclusive Note Off and notes extending beyond the cycle.

Prepared publication includes a rounded sample period. A feasible old-cycle
boundary becomes the new origin, so length changes do not start partway through
the new phrase. The same selection maps transport and existing clip segments.
Requests arriving after one track rendered the transition wait for the complete
transition before another plan is selected. Existing fixed-period cases remain.
Dynamic tests cover extension, contraction, unequal track lead, trailing rests,
failure preservation and Stop using actual plugin input samples and master output.
MML endpoints use existing converter stages with calculation-only program markers;
marker placement and longest-part selection remain JavaScript, outside the audio path.

A small upstream control/audio handoff API for these primitives could replace
this shim while leaving MML generation and named-part composition in script.
The app's loop / loop-update E2E cases exercise actual plugin input and output.

Status: not sent.
