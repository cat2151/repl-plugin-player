# Frozen disk cache loses the attack on repeated long loops

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告. This is a draft, not an external submission.

Load a project with a drum hit at its beginning, freeze the tracks, and repeat a
transport loop longer than the approximately one-second disk lookahead. Even
after mapping FrozenTrackManager's cache position and split reads to the loop,
the beginning of the drum hit can disappear at each wrap. FrozenTrackAudio's
forward loader has evicted the head and read(..., false) fills it with zeros.

Keep a bounded loop-head window resident independently of forward prefetch.
Simply prefetching into the existing modulo-indexed slots can evict the tail
or head when their chunk IDs collide. A separate bank preserves both and keeps
the callback nonblocking. Retain the reader hazard handshake for both banks.

The local regression writes test-owned stereo samples and compares nonblocking
and blocking reads immediately after simulated wraps, with no loader work allowed
at the wrap. It covers long zero/nonzero, fractional-chunk loops, channel latency
shifts, repeated wraps, changed ranges and disabled looping.

The unmodified reader fails this sample test at position 0; the repaired reader
passes. A read-only load of a real four-track archive, with all four tracks frozen,
produced identical first-100-ms drum samples at three wraps of a 384001-frame loop
at 48 kHz (maximum sample differences `[0, 0]`). Cold initial Play in this harness
still had a silent first window; the repeat-wrap repair does not address that
separate startup transition.
