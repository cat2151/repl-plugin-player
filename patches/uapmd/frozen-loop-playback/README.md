# Frozen track loop playback

UAPMD `4ec5c3b1c30130fdd3b7d4b375082273247e561f` wraps ordinary timeline clips
but reads frozen audio at the monotonically increasing callback transport position.
After the first pass it reads the cache tail and then silence. The reproduced
one-second loop had peaks of about 0.28 on its first pass and zero on later passes.

This repair wraps both the cache read position and its loader position. It splits
reads at loop endpoints, including nonzero ranges and multiple wraps inside one
callback. Offline rendering ignores the realtime loop as upstream already does.
No disk wait, allocation, plugin processing, or project-format change is added to
realtime playback. The upstream nonblocking loader can still produce temporary
silence if audio at a newly requested position is not resident; this repair does
not claim to eliminate all cache misses.

The include in `native/CMakeLists.txt` generates only
`target/shim/frozen-loop-playback/FrozenTrackManager.cpp`. Dependency checkouts and
other repairs are unchanged. Fresh checkouts and existing build caches take the
same path. Every source anchor and CMake source entry must match; otherwise
configuration stops with the repair name, target file, failed condition, upstream
revision and this document. Inspect upstream differences before changing anchors.
Remove this include and directory only after verifying an upstream fix handles
both loop-position mapping and reads crossing a callback boundary.

Run `python -B patches/uapmd/frozen-loop-playback/test_generate.py`, the normal
`python scripts/build.py`, then build and run CMake target `frozen_loop_regression`.
`python scripts/freeze-test.py --loop` checks audible loop playback as well as
freeze/edit/off/save/restart restoration using test-owned material outside the repo.
Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
