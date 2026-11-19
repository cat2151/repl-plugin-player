# MIDI-only CLAP instruments receive unsupported native notes

Affected UAPMD revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
`PluginInstanceCLAP.Events.cpp` emits CLAP_EVENT_NOTE_ON/OFF for all incoming UMP notes without checking the input note port dialect.
sforzando CLAP 2.1.2.4 advertises MIDI-only input (supported/preferred dialect 2); its SFZ state restores but these notes do not sound.

Collect supported input dialects before activation, emit native notes for native-capable ports and MIDI bytes for MIDI-only ports. Both-capable ports should receive exactly one note. Preserve channel, port, timestamp, velocity, Note On/Off semantics and supported CC messages. Keep the old native behavior when the note-port extension is absent.

The port must not be taken from the UMP group unchecked: `SequencerTrack::findAvailableGroup` gives a second instance on a track group 1, and a plugin with one input note port has no port 1. Use the group as the port only when the plugin has that port, else port 0.

Local reproduction and tests are in this directory. This is a draft report, not sent upstream.
