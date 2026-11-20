# Headless CLAP host

Validated upstream: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.

## Cause

`RemidyCLAPHost::paramsRescan` walks every parameter of the instance. For
`CLAP_PARAM_RESCAN_VALUES` it calls `paramsGetValue` and notifies each value; the
first call on an instance also builds the parameter list (`paramsGetInfo` per
parameter and repeated `paramsValueToText` for stepped parameters). A plugin that
rescans from `stateLoad` and again from `onMainThread` therefore blocks the main
thread for every state load. With Six Sines (2554 parameters) one state load cost
about 1.5 s, and about 4 s on a new instance.

Instances created by evolve (`native/evolve_api.hpp`) are never shown in a
track, UI or script, so nobody reads their parameter list or values.

## Repair

- `headless_scope.hpp`: a `thread_local` depth and the RAII `repl_headless::Scope`.
  evolve holds one around `addPluginToPreparedTrack`. That call runs on the main
  thread and creates the instance synchronously in the same call stack
  (`EventLoop::runTaskOnMainThread` runs inline on main), so the mark reaches the
  host construction. If creation ever moves to another thread, the instance
  silently gets the ordinary host: slower, not broken.
- `headless_host.hpp`: `HeadlessCLAPHost : RemidyCLAPHost`, overriding only
  `paramsRescan` with a no-op. `requestCallback` (the plugin may do other work in
  `onMainThread`) and `paramsRequestFlush` (needed for sound) are inherited.
- `apply.py`: reads clap-note-dialect's generated `PluginFormatCLAP.cpp`, replaces
  the single `auto host = std::make_unique<RemidyCLAPHost>();` with a choice by the
  depth, adds the two includes before `namespace remidy {`, and writes
  `target/shim/headless-clap-host/PluginFormatCLAP.cpp` (only if the text changed).
- `apply.cmake`: runs `apply.py` and swaps exactly one remidy source entry for the
  output.

## Application conditions

- `native/CMakeLists.txt` includes `apply.cmake` directly after
  `clap-note-dialect/CMakeLists.cmake`; this repair consumes that repair's output
  and leaves its contents and the UAPMD checkout untouched.
- The input contains exactly one host-construction anchor and exactly one
  `namespace remidy {` line, and remidy's `SOURCES` contains the input exactly once.
- Track instances are created outside the scope and keep `RemidyCLAPHost` unchanged.

## Reapplication

The input, `apply.py` and both headers are `CMAKE_CONFIGURE_DEPENDS`, so any change
re-runs configuration. Existing caches and fresh checkouts regenerate the same file
by the same path; an unchanged output is not rewritten.

## Failure

A missing or repeated anchor makes `apply.py` exit non-zero with the repair name,
target file, condition, UAPMD revision and this document; CMake stops with
`FATAL_ERROR`. A missing or repeated source entry stops CMake with the same items.
Compare the clap-note-dialect output with upstream `src/clap/PluginFormatCLAP.cpp`
and `HostClasses.hpp` (the `paramsRescan` signature) before updating the anchors.

## Tests

- `python -B patches/uapmd/headless-clap-host/test_apply.py`: one replacement,
  includes before the namespace, input untouched, idempotent reapply, rejection of
  the unpatched text, and missing/repeated anchors.
- `python -B scripts/evolve-fixed-test.py --topologies evolve --workers 6 --seconds 30 --sounds 3`:
  evolve's Six Sines requests should no longer stall the UI.
- `target/release/repl-plugin-player.exe --e2e script-ui` and
  `python -B patches/uapmd/parameter-metadata/regression.py`: track instances still
  report parameters.

## Removal

Remove the include line, this directory and the `Scope` in
`native/evolve_api.hpp` when upstream lets a host skip value rescans for
unobserved instances, or no longer walks every parameter on
`CLAP_PARAM_RESCAN_VALUES`.

Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
