# Dedicated evolve graphs in the existing worker batch

The A/B adapter originally rendered its two unpublished graphs in serial after callbacks. Fixed-state measurement showed a 34.6 ms callback with two 17 ms graphs, exceeding the 21.33 ms device block. This repair routes those graphs through the engine's existing parallel jobs. Rust still owns event timing, gain envelopes and silent-slot ownership; C++ supplies the UAPMD graph connection.

The repair applies only while the evolve adapter is registered. Without registration the selected graph and 80% worker deadline are unchanged. During evolve, jobs use the actual block deadline: after callbacks only apply Rust gains, so the ordinary early cutoff can discard otherwise usable audio. This does not guarantee that arbitrary plugins finish within the device period.

The graph override alone does not establish uninterrupted playback: fixed-state runs still observe worker-discard blocks and callback deadline misses. A master-output snapshot can retain its last successful samples when a worker batch is discarded; nonzero snapshot peaks alone do not prove that the device received every block. Use worker-fault evidence as well as output observations. Delivered-block counts and elapsed time must come from the same final observation.

Verified upstream revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`. The input is the existing dag-builtins generated engine translation unit. Existing generators and outputs are not modified. `apply.cmake` replaces that engine translation unit with a separate copy in `target/shim/evolve-graphs/`; it never edits the dependency checkout. Both fresh clones and existing caches run the same guarded generator. Unchanged output keeps its timestamp.

If an anchor or target-source check fails, configuration stops with the repair name, source, condition, upstream revision and this document. Compare upstream and the current generated dag-builtins engine before changing anchors. Do not bypass validation. Check upstream for an equivalent graph override and evolve deadline policy before updating or removing the repair; remove its include and the adapter hook together when upstream supplies that contract.

Run `python -B patches/uapmd/evolve-graphs/test_generate.py target/shim/dag-builtins/SequencerEngine.cpp`, build/run `evolve_graphs_regression`, then use `scripts/evolve-fixed-test.py --topologies evolve --workers 6 --seconds 90` for real plugin events, preparation/fade callback timing, worker faults and output. Ordinary-mode, archive restoration and standalone deployment checks remain required for integration.

Author report status: not sent. The draft is [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
