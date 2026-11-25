# Routed builtin nodes in a full DAG

Affected UAPMD revision: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.
The DAG builder accepts generic builtin nodes, but its runtime only creates contexts for plugin nodes and then processes all builtins as a final serial chain.
An instrument → GainNode → effect edge can therefore be rejected as a cycle, and audio routed from a builtin has no runtime output. This breaks the layer gains and post-sum effects in editor exports.

The repair puts generic builtins in the compiled DAG and keeps only `builtin:track_gain` as the implicit final fader. A custom graph processes every routed builtin once and ignores disconnected builtins. The unchanged simple-chain processing path retains its serial behavior.
`generate.py` verifies the normalized source SHA256, then emits a replacement only under `target/shim/dag-builtins`. The checkout, dependency version and other patches are untouched.
The include in `native/CMakeLists.txt` reruns generation when its inputs change. Matching fresh clones regenerate the repair; cached builds retain identical output timestamps.
The generated graph uses the existing note-monitor header, preserving scope and input-event observations.
An assigned instance group is also normalized to local group 0 immediately before processing: it selects the instance within a track, and is not CLAP input port 1..15. Unassigned groups retain their original port addressing. This is limited to the custom DAG path used by exported projects.
The engine's processing-group cache prefers an explicitly restored or selected assignment over the optional MIDI-CI device group. Automatic allocations on newly created nodes retain their previous device routing, including browser auditions. Overrides are removed with their instances. Instance-targeted queued events are already addressed to that instance; the custom DAG drains them independently of the clip's group filter. Both source inputs are hash-validated before either generated replacement is written.
Group selection retains Utility messages, including JR Timestamp deltas, in their original order. They have no destination group: filtering their reserved group nibble drops within-block timing for group 1..15. A 0.1-second group-1 note can then start and stop together inside an 8192-frame offline block, while a 1-second note stops at frame 40960 instead of approximately 48000 at 48 kHz. `routing.hpp` owns the repaired filter used by the generated DAG and the event-timing regression; note selection and assigned-port normalization remain separate.
On upstream drift, generation stops and names the source, condition, revision and this README. Inspect for an upstream fix before changing the baseline. Remove the repair and include when upstream passes the regression without it.

`python -B patches/uapmd/dag-builtins/test_generate.py ../uapmd/source/uapmd-graph/src/node-graph/AudioPluginFullDAGraph.cpp` checks nonmutation, deterministic reapplication and drift rejection.
Build and run CMake target `dag_builtins_regression`: two parallel gains must sum before the implicit track fader, and a disconnected gain must not alter output. Its event-timing cases select groups 0, 1, 15 and unassigned input from mixed MIDI 1/2 events, preserving JR deltas even between discarded events, complete messages and the source buffer; port normalization must leave timestamps unchanged. The editor's `scripts/uapmd-effects-test.py` verifies the real archive's restored edges and effect states.

Upstream report: not sent; see `UPSTREAM_REPORT.md`.
