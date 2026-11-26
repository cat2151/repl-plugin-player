# CLAP SysEx input

Validated upstream: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.

## Cause

A CLAP instance's input events are made only by `ump_input_dispatcher.process`
(`source/remidy/src/clap/PluginInstanceCLAP.cpp`) and by parameter changes.
`TypedUmpInputDispatcher::process` (`source/remidy/src/UmpInputDispatcher.cpp`)
handles UTILITY, MIDI1 and MIDI2 messages only, so a UMP SysEx7 message never
reaches the plugin, and there is no other way to put a `CLAP_EVENT_MIDI_SYSEX`
into the next `process()`.

## Repair

The bridge's `repl_clap_sysex(plugin, bytes, size)` queues one SysEx message
(copied) for the next `process()` of the live CLAP instance whose `clap_plugin*`
is `plugin` (a track instance's comes from `repl_clap_plugin`). No plugin is
named here; what is sent is decided in Rust.

- `clap_sysex_input.hpp` / `sysex_input.cpp` (built in `remidy`): a queue per
  `clap_plugin*`. `drain` runs on the audio thread, takes the queue with
  `try_lock` (a busy queue is read on the next block) and pushes each message
  as `CLAP_EVENT_MIDI_SYSEX` on port 0 at time 0. `clap::helpers::EventList`
  copies the bytes and keeps them until it is cleared after `process()`.
- `apply.cmake`: reads clap-note-dialect's generated
  `PluginInstanceCLAP.cpp`, writes a copy to `target/shim/clap-sysex-input/`
  with three insertions (the include, `forget` at the start of the destructor,
  `drain` before `ump_input_dispatcher.process(process);`), and puts the copy
  in `remidy` in place of the generated file. The instance's plugin pointer
  comes from `clap-plugin-handle`.

A message waits while the instance does not process (not activated, engine
stopped), so it is never given to a plugin before `activate()`.

## Application conditions

- `uapmd-host/CMakeLists.txt` includes `apply.cmake` after
  `clap-note-dialect/CMakeLists.cmake` (it reads that repair's generated file)
  and after `clap-plugin-handle/apply.cmake` (the bridge uses its pointer).
  The UAPMD checkout and the other repairs are untouched.
- The generated `PluginInstanceCLAP.cpp` has each of
  `#include <umppi/umppi.hpp>`, `PluginInstanceCLAP::~PluginInstanceCLAP() {`
  and `ump_input_dispatcher.process(process);` (with their indentation and
  line end) exactly once, and `remidy` builds that file exactly once.

## Reapplication

The generated input is `CMAKE_CONFIGURE_DEPENDS`, so regenerating it re-runs
configuration. The copy is written only when its content changes.

## Failure

A missing or repeated anchor, or a `remidy` source list without the generated
file, stops CMake with `FATAL_ERROR` naming the repair, target file, condition,
UAPMD revision and this document. Compare the generated file with upstream
`src/clap/PluginInstanceCLAP.cpp` (where input events are filled before
`plugin->process`) before updating the anchors.

## Tests

- `python scripts/e2e.py patch-timbre`: its cell for a plugin that takes a
  patch as SysEx sends two programs of one bank into a track instance through
  this queue and compares the output.
- `rg -i` for the plugin's name and ID over `patches/uapmd` and `native` finds
  nothing: no plugin is named on this side.

## Removal

Remove the include line and this directory when UAPMD can deliver SysEx to a
CLAP instance (UMP SysEx7 converted to `CLAP_EVENT_MIDI_SYSEX`, or an API that
queues input events for the next `process()`), and switch the bridge to that.

Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
