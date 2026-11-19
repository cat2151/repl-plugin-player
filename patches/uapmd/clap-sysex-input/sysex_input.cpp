// Built in the remidy target, which has the CLAP and clap-helpers headers.
#include "clap_sysex_input.hpp"
#include <clap/events.h>
#include <clap/helpers/event-list.hh>
#include <mutex>
#include <unordered_map>
#include <vector>

namespace {
    using Messages = std::vector<std::vector<uint8_t>>;
    std::mutex queue_mutex;
    // Only plugins with messages not yet given to process() have an entry.
    std::unordered_map<const void*, Messages> queue;
}

void repl_clap_sysex_input::send(const void* plugin, const uint8_t* bytes, size_t size) {
    std::lock_guard lock(queue_mutex);
    queue[plugin].emplace_back(bytes, bytes + size);
}

void repl_clap_sysex_input::drain(const void* plugin, clap::helpers::EventList& events) {
    Messages messages;
    {
        // The audio thread never waits for the control thread: a busy queue is
        // read on the next block.
        std::unique_lock lock(queue_mutex, std::try_to_lock);
        if (!lock.owns_lock() || queue.empty()) return;
        auto found = queue.find(plugin);
        if (found == queue.end()) return;
        messages = std::move(found->second);
        queue.erase(found);
    }
    for (auto& message : messages) {
        clap_event_midi_sysex event{};
        event.header.size = sizeof(event);
        event.header.time = 0;
        event.header.space_id = CLAP_CORE_EVENT_SPACE_ID;
        event.header.type = CLAP_EVENT_MIDI_SYSEX;
        event.header.flags = 0;
        event.port_index = 0;
        event.buffer = message.data();
        event.size = static_cast<uint32_t>(message.size());
        events.push(&event.header);
    }
}

void repl_clap_sysex_input::forget(const void* plugin) {
    std::lock_guard lock(queue_mutex);
    queue.erase(plugin);
}
