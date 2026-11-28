# UMP SysEx7 is not delivered to CLAP plugins

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: resolved upstream in `277aa85468795267d6cf64953af587ea18c7f653`
(UMP SysEx7 is reassembled in `TypedUmpInputDispatcher` and given to CLAP as
`CLAP_EVENT_MIDI_SYSEX` by `onSysEx7`). Not to be sent.

The repair stays until the app sends its SysEx through that path
(see README.md, Removal); no further upstream request is needed for it.
