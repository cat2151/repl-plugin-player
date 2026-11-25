# Frozen loop head prefetch

Validated upstream: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.
Report status: **未報告**; [author report draft](UPSTREAM_REPORT.md).

The existing frozen-loop-playback repair wraps sample positions, but the upstream
disk loader only retains a forward window. In a loop longer than that window,
the head has been evicted when the audio callback wraps. Nonblocking reads return
silence until the loader catches up, clipping the first drum transient.

This repair adds an independent, bounded bank with the same channel-shift-aware
window at the loop head. The ordinary bank cannot evict it. The loader fills the
current window first and then the head; both retain the existing slot publication
and reader hazard protocol. Callbacks only publish an atomic head position and
try an additional resident bank on a miss: no disk reads, allocation, or waits.
The additional memory is one existing lookahead window per frozen track, including
latency spread, independent of project duration. Initial playback or a newly edited
loop can still miss before its first prefetch completes; steady repeated wraps
retain the head. Offline reads retain their blocking fallback and ignore realtime
loop state. Files and the project format are unchanged, so no data migration is needed.

`native/CMakeLists.txt` includes this repair after freeze-monitor. Configure generates
FrozenTrackAudio.cpp/.hpp and FrozenTrackManager.cpp under
`target/shim/frozen-loop-prefetch`, replacing only those compilation units.
Dependency checkouts and existing repairs are unchanged. Fresh clones and existing
caches run the same checked generator. Unexpected source anchors or target entries
stop configuration with the repair name, file, condition, revision, and this path.
Inspect the reported upstream source and preceding generated manager before updating
anchors; do not bypass a failure. Remove the include and this directory when an
upstream fix provides resident audio across long, nonzero and nonaligned loop wraps.

Checks: `python -B patches/uapmd/frozen-loop-prefetch/test_generate.py`, normal
`python scripts/build.py`, then `cmake --build build --config Release --target
frozen_loop_prefetch_regression` and `build/out/frozen_loop_prefetch_regression.exe`.
The regression compares exact realtime samples with disk-backed offline reads at
each wrap, with a loop exceeding the lookahead, stereo channel shifts, repeated
wraps, loop changes, and disabled looping. Its temporary audio is outside the repo.

`frozen_loop_prefetch_baseline` runs the same comparison with the upstream reader
and is expected to fail (`loop-head audio missing at 0`). The repaired target passes.
`python -B patches/uapmd/frozen-loop-prefetch/check_project.py X:/music/song.uapmdz
--track 2` loads the supplied archive read-only, freezes every clip track and uses
the real device and scope callback to compare the selected track's first 100 ms
at three wraps. It checks the archive hash and removes isolated native state after
the child exits. The track index is explicit; this test produces audible playback.
The current four-track project (384001-frame loop at 48 kHz, drum track 2) produced
identical wrap-head peaks of `0.0308660846`, with maximum sample errors `[0, 0]`.
The cold initial start in that harness produced a zero peak even with a brief
stopped-engine warmup; it is reported separately and remains outside this repair.
