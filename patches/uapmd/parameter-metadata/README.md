# CLAP parameter metadata

UAPMD 277aa85468795267d6cf64953af587ea18c7f653 hardcodes public JS
`isReadonly=false`; its CLAP `PluginParameter` constructor does not preserve
`CLAP_PARAM_IS_READONLY`. Script controls must not infer writability from
automatable or readable. CLAP permits live user changes regardless of automatable.

`generate.py` copies upstream `PluginInstanceCLAP.Parameters.cpp` to `target/shim/parameter-metadata`
and replaces that one remidy source, observing
real CLAP info during synchronous metadata enumeration. `metadata.hpp` captures
only the requested index on the control thread. Native `parameter-safety` checks
the vector index and stable CLAP ID and returns raw flags, plain ranges and an
epoch. Unknown formats, failed info queries and inconsistent indexes remain
read-only. The public JS metadata also uses this result, replacing the fabricated
flag, and exposes the existing discrete metadata for snippet selection.

Every metadata clear/refresh changes the epoch; destruction removes the owner.
UI binding fingerprints include it, so even an identical replacement instance or
metadata rescan requires an explicit new `repl.ui.show`. Values are read fresh
through the public parameter API, outside the Rust host callback.

The checkout is never written. Both fresh and cached builds regenerate the
copy from upstream. Strict unique anchors reject
unknown shapes, including an upstream readonly implementation. On failure CMake
reports the supplement, generated target file, failed anchor, upstream revision
and this document. Inspect the original and generated source, run the regression,
and check upstream before changing anchors; never bypass the check.

Verification: `python patches/uapmd/parameter-metadata/regression.py`; build and
run `parameter_metadata_regression` in the existing build directory; normal
`python scripts/build.py`; `target/release/repl-plugin-player.exe --e2e script-ui`. The last
check uses installed Surge silently, pointer input through egui, and restores the
original parameter value in all cases. It checks both external changes and
script writes, snippet insertion, window lifetime, close/reopen and Stop.

Remove when upstream exposes trustworthy readonly/stepped metadata and instance/
metadata invalidation sufficient for script binding. Verify those contracts and
regressions first, then remove this directory, its CMake include/native hooks and
AGENTS listing. Upstream report: **未報告**; draft in `upstream-report.md`.
