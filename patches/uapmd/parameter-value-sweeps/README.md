# Parameter value sweeps

Validated upstream: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.

## Cause

A patch (state) load on a track's plugin walks every parameter value on the main
thread three times:

- `RemidyCLAPHost::paramsRescan(CLAP_PARAM_RESCAN_VALUES)` calls `getParameter`
  and `notifyParameterValue` for every parameter. Six Sines rescans from
  `stateLoad` and again from `onMainThread`, so this runs twice per load.
- `TimelineFacadeImpl::refreshPluginParameterCache` (after every plugin state
  mutation) calls `getParameterValue` for every parameter, twice, to fill
  `plugin_parameter_values_`.

Six Sines (2554 parameters, about 0.29 ms per `get_value` inside the plugin)
therefore blocked the UI for about 3 s per patch cell on the same instance.

Nobody needs those walks: every value reader asks the plugin on demand
(`getParameterValue` → `paramsGetValue`), `plugin_parameter_values_` has no reader,
and the listeners of the notifications are either unread (`UapmdJSRuntime`'s
parameter update queue) or not created by this app (virtual MIDI devices).

## Repair

- `apply.py` writes two files to `target/shim/parameter-value-sweeps/`:
  - `HostClasses.cpp` from clap-note-dialect's generated copy, with the
    `CLAP_PARAM_RESCAN_VALUES` branch of `paramsRescan` removed. `ALL`, `INFO`
    and `TEXT` rescans still rebuild the metadata.
  - `TimelineFacadePlugins.cpp` from UAPMD, with `refreshPluginParameterCache`
    erasing the instance's cache entries instead of reading every value.
- `apply.cmake` swaps exactly one `remidy` and one `uapmd-engine` source entry for
  the outputs, and adds the original `src/sequencer` to `uapmd-engine`'s private
  include directories (the copy includes `TimelineFacadeImpl.hpp` relatively).

Not repaired: `TimelineFacadeImpl::pluginInstanceAdded` (inline in
`TimelineFacadeImpl.hpp`) still builds the parameter list and reads every value
once when an instance is created. The list must stay eager: CLAP's
`getParameter`, `setParameter` and `enqueueParameterRT` index `parameter_ids`
without building it, and `enqueueParameterRT` runs on the audio thread where
`params.get_info` may not be called. Dropping only the value loop would need all
seven translation units that include the header copied to keep one class
definition.

## Application conditions

- `native/CMakeLists.txt` includes `apply.cmake` after
  `clap-note-dialect/CMakeLists.cmake` and `headless-clap-host/apply.cmake`; this
  repair consumes clap-note-dialect's output and leaves that repair's files, the
  other repairs and the UAPMD checkout untouched.
- Each input contains its anchor (the whole original block) exactly once, and each
  target's `SOURCES` contains the input exactly once.

## Reapplication

Both inputs and `apply.py` are `CMAKE_CONFIGURE_DEPENDS`, so any change re-runs
configuration. Existing caches and fresh checkouts regenerate the same files by
the same paths; an unchanged output is not rewritten.

## Failure

A missing or repeated anchor makes `apply.py` exit non-zero with the repair name,
target file, the anchor's first line, UAPMD revision and this document; CMake
stops with `FATAL_ERROR`. A missing or repeated source entry stops CMake with the
same items. Compare the input with the anchors in `apply.py` and check whether
upstream still walks the values before updating them.

## Tests

- `python -B patches/uapmd/parameter-value-sweeps/test_apply.py`: both walks are
  gone, metadata rescans stay, input untouched, idempotent reapply, rejection of
  the unpatched text, and missing/repeated anchors.
- `python -B scripts/e2e.py patch-rescan`: Six Sines patch cells on the same track
  instance stall the UI for less than 150 ms (`PATCH_STALL_MS`; without the
  repair about 3 s), and the parameter values read after a patch change are the
  new patch's values.
- `python scripts/e2e.py patch-timbre`, `target/release/repl-plugin-player.exe --e2e script-ui`,
  `python -B patches/uapmd/parameter-metadata/regression.py`: sound and parameter
  display still follow the patch.

## Removal

Remove the include line and this directory when upstream no longer walks every
value on `CLAP_PARAM_RESCAN_VALUES` and no longer refreshes an unread value cache
after a state load.

Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
