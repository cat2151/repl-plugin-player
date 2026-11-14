#pragma once
#include <uapmd-graph/uapmd-graph.hpp>
#include <umppi/umppi.hpp>

namespace repl_dag {
inline uint8_t processingGroup(uint8_t assigned, uint8_t device) {
    return assigned <= 15 ? assigned : device;
}
// An assigned UMP group addresses the instance on its track, not CLAP port 1..15.
inline void normalizeInstanceGroup(remidy::EventSequence& input, uint8_t assigned) {
    if (assigned == 0xff) return;
    auto* words = static_cast<uint32_t*>(input.getMessages());
    const auto count = input.position() / sizeof(uint32_t);
    for (size_t offset = 0; offset < count;) {
        const auto type = words[offset] >> 28;
        const auto size = static_cast<size_t>(umppi::umpSizeInInts(type));
        if (!size || offset + size > count) return;
        if ((type >= 1 && type <= 5) || type == 13)
            words[offset] &= 0xf0ffffffu;
        offset += size;
    }
}
}
