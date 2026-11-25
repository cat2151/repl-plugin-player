# Master scope

evolve uses unpublished instruments, and an individual instrument tap cannot show
the crossfaded master output. The realtime dispatcher return is wrapped to observe
the final stereo output after master gain, soft clipping and mute processing.
Offline rendering is unaffected. Target -2 selects master; -1 disables capture.
The existing bounded scope queue is reused, with no allocation or locks in the tap.
Master frames use a 20 ms window without pitch similarity analysis.

Validated upstream: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.
The checkout is never modified. CMake regenerates
`target/shim/master-scope/RealtimeSequencer.cpp` from the checkout on configure,
including after reclone and with an existing build cache. An absent or duplicate
anchor, or a missing target source, stops configuration with patch name, file,
revision and this document's location. On failure inspect the upstream callback
and target source list before updating the anchor; do not bypass validation.

Validation: `python -B patches/uapmd/master-scope/test_generate.py`, Rust scope
tests, release build, `python -B scripts/e2e.py scope-master`,
`python -B scripts/evolve-mode-test.py` and `python -B scripts/master-scope-test.py`.
The performance comparison uses the external hashed fixture prepared by
`evolve-fixed-test.py`, keeping the same voice while selecting off/on/on/off.
Remove this patch and its CMake
include when upstream exposes a final realtime output observer, then move the
bridge capture call to that observer and repeat output/performance checks.

Author report: not sent; draft in `UPSTREAM_REPORT.md`.
