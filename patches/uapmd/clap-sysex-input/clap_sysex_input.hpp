#pragma once
#include <cstddef>
#include <cstdint>

namespace clap { namespace helpers { class EventList; } }

namespace repl_clap_sysex_input {
    // Queues one MIDI SysEx message (F0 ... F7) for the next process() of the
    // CLAP instance whose `clap_plugin*` is `plugin`; the bytes are copied.
    // Messages wait while the instance does not process.
    void send(const void* plugin, const uint8_t* bytes, size_t size);
    // Audio thread, inside process(): moves the messages queued for `plugin` into
    // `events` as CLAP_EVENT_MIDI_SYSEX on port 0 at time 0. The list copies the
    // bytes and keeps them until it is cleared after process().
    void drain(const void* plugin, clap::helpers::EventList& events);
    // Drops what is still queued for `plugin`, before the plugin is destroyed.
    void forget(const void* plugin);
}
