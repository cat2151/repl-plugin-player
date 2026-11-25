# CLAP plugin handle

Validated upstream: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.

## Cause

A plugin's own CLAP extension (one the host does not know) is reached only
through the instance's `clap_plugin*`. UAPMD returns it from
`AraHandleExtension::nativeHandle(CLAPPlugin)`, which exists only under
`UAPMD_HAS_ARA`; this build has `UAPMD_ENABLE_ARA=OFF`. Without ARA,
`PluginInstanceCLAP::plugin` is private and has no accessor, and
`RemidyAudioPluginInstance::rawInstance()` is not reachable from
`AudioPluginInstanceAPI`.

## Repair

The bridge's `repl_clap_plugin(instanceId)` returns the instance's
`clap_plugin*` (null when the instance is missing or not a remidy CLAP
instance). No plugin is named here; what the pointer is used for lives in Rust.

- `clap_plugin_handle.hpp`: the two functions the bridge and the units below share.
- `instance.cpp.in`, built in `uapmd-plugin-hosting`: `AudioPluginInstanceAPI*`
  to `RemidyAudioPluginInstance::rawInstance()` by `dynamic_cast`.
- `clap.cpp.in`, built in `remidy` with the same CLAP class header as every CLAP
  unit (clap-note-dialect's generated `PluginFormatCLAP.hpp`): `PluginInstance*`
  to `PluginInstanceCLAP` by `dynamic_cast`, then the private `plugin` proxy's
  `clapPlugin()`. The member pointer comes from an explicit template
  instantiation, where access checks do not apply, so the class is not edited.
- `apply.cmake`: checks the anchors, writes both units to
  `target/shim/clap-plugin-handle/` with `configure_file`, and adds each to its
  target with `target_sources`.

## Application conditions

- `native/CMakeLists.txt` includes `apply.cmake` after
  `clap-note-dialect/CMakeLists.cmake` (it reads that repair's generated header);
  the UAPMD checkout and the other repairs are untouched.
- The generated header has `class PluginInstanceCLAP : public PluginInstance {`
  and `std::unique_ptr<CLAPPluginProxy> plugin;` exactly once each, and upstream
  `RemidyAudioPluginInstance.hpp` has
  `class RemidyAudioPluginInstance : public AudioPluginInstanceAPI {` and
  `remidy::PluginInstance* rawInstance() const` exactly once each.

## Reapplication

Both inputs and both `.cpp.in` files are `CMAKE_CONFIGURE_DEPENDS`, so a change
re-runs configuration. Fresh checkouts and existing caches write the same files
by the same path; `configure_file` leaves an unchanged output untouched.

## Failure

A missing or repeated anchor stops CMake with `FATAL_ERROR` naming the repair,
target file, anchor and count, UAPMD revision and this document. Compare the
generated header with upstream `src/clap/PluginFormatCLAP.hpp` (the member's
name and type, `CLAPPluginProxy::clapPlugin`) and
`RemidyAudioPluginInstance.hpp` before updating the anchors. A changed member
type that keeps the anchor text fails at compile time in `clap.cpp`.

## Tests

- `python scripts/e2e.py patch-timbre`: its cell for a plugin with its own preset
  extension loads two presets into a track instance through this pointer and
  compares the output.
- `rg -i` for the plugin's name and ID over `patches/uapmd` and `native` finds
  nothing: no plugin is named on this side.

## Removal

Remove the include line and this directory when UAPMD exposes the CLAP plugin
pointer of an instance without ARA (for example `nativeHandle` outside
`UAPMD_HAS_ARA`), and switch the bridge to that.

Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
