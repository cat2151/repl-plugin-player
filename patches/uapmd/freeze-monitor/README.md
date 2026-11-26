# Frozen playback observations

Validated upstream: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.
Frozen playback bypasses AudioPluginNodeImpl, where this app observes keyboard
notes and scope waveforms. The cached-audio processor must provide those
observations without running plugin DSP again.

The processor observes the scheduled track UMP and cached track output. The scope
therefore shows the rendered track mix, including effects, for any selected plugin
on that track; intermediate plugin audio is not present in the freeze cache.
The existing bounded note monitor is owned by the frozen playback state and is
released on stopped/live processing, track retirement or state destruction. No
format change, disk wait, heap allocation in the callback, or plugin processing
is introduced.

The CMake include runs after frozen-loop-playback and consumes its generated cpp.
It generates a new cpp, PlaybackState header, and umbrella header under
`target/shim/freeze-monitor`. Other umbrella includes point to the original
headers. The public include directory ensures consumers share one class layout.
Dependency checkouts and existing repairs are unchanged. Existing caches and
fresh checkouts regenerate by the same path.

The app's `uapmd-host/freeze_transport.hpp` polls on its control thread. A pending
freeze automatically pauses transport beyond the complete score's content end,
preserving the playhead, then uses the normal tail/quiet notification to render.
Valid loops, live device input, empty scores and ordinary playback without pending
freeze are left running. An audible tail still drains normally before rendering.

Run `python -B patches/uapmd/freeze-monitor/test_generate.py`, the normal release
build, `python -B scripts/e2e.py scope-load`, and
`python -B scripts/freeze-test.py --loop`. The latter uses test-owned isolated data
and checks automatic freezing without Stop; scope-load checks held keys and a
nonzero pitched scope waveform during frozen playback.

Missing/repeated anchors or missing CMake source entries stop configuration with
the repair name, affected source, failure condition, upstream revision and this
document. Inspect upstream differences and generated include paths before updating
anchors. Remove this include and directory after upstream supplies a cached-output
observation hook, adapting the app to that hook and retaining the playback tests.

Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
