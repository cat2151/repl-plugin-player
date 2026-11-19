# UMP SysEx7 is not delivered to CLAP plugins

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告; draft only.

Some CLAP instruments take patches as MIDI SysEx (for example a single voice
dump into the edit buffer). In remidy, CLAP input events come from
`ump_input_dispatcher.process` (`source/remidy/src/clap/PluginInstanceCLAP.cpp`)
and parameter changes. `TypedUmpInputDispatcher::process`
(`source/remidy/src/UmpInputDispatcher.cpp`) switches on UTILITY, MIDI1 and MIDI2
message types only, so a SysEx7 (message type 3) UMP sent to the instance is
dropped, and a host application has no other way to add a
`CLAP_EVENT_MIDI_SYSEX` to the next `process()`.

Suggestion: assemble SysEx7 packets (start/continue/end) in the dispatcher and
emit `CLAP_EVENT_MIDI_SYSEX` (with `F0`/`F7` added) for plugins whose note port
accepts `CLAP_NOTE_DIALECT_MIDI`, keeping the buffer alive until `process()`
returns (`clap::helpers::EventList::push` already copies it).

The app works around this by building a copy of `PluginInstanceCLAP.cpp` that
drains an app-side queue of SysEx messages into `events_in` before the UMP
dispatcher runs.
