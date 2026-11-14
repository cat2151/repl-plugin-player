# Full DAG generic builtins are not in the compiled graph

On UAPMD `4ec5c3b1c30130fdd3b7d4b375082273247e561f`, `rebuildCompiledState` creates runtimes only from `state.nodes` (plugins), while generic builtins remain in `state.builtin_nodes` and run serially after the DAG.
Create a GainNode and connect input → gain → plugin → output: the gain-to-plugin connection fails or the routed source has no runtime.

Compile generic builtins with plugins; exclude the implicit track fader, which must still process the assembled output once. Preserve the pre-customization simple-chain behavior. Regression: parallel gains of 0.5 and 0.25, followed by an implicit 0.2 fader, produce 0.15 for input 1; an unconnected gain must not change the output.

The track's assigned instance group is another routing boundary: after selecting the instance, remap to local group 0 before processing. Otherwise group 1 becomes CLAP port 1, and a single-port instrument is silent. The device-free SFZ test reproduces group 1 silence with correct saved state; unassigned multiport inputs must retain their original group values.

`appendEventsForGroup` also compares the group nibble of Utility messages. JR Timestamps have no destination group, so selecting group 1..15 drops them and collapses within-block note timing. With a hand-built SMF2 clip, one Six Sines instance and 8192-frame freeze blocks at 48 kHz, a group-0 note ends at frame 4800, while the same 0.1-second group-1 note starts and stops at frame 0 and renders silence. A 1-second note ends at frame 47999 on group 0 and frame 40960 on group 1. Preserve Utility messages in order while filtering group-addressed messages; the local regression covers mixed groups, MIDI 1/2 messages and timestamp preservation through port normalization.

This directory contains a source-generation repair and regression. Draft report only; not sent upstream.
