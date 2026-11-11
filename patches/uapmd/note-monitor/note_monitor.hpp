#pragma once
#include <array>
#include <atomic>
#include <cstdint>
#include <cstddef>
#include "../loop-playback/event_trace.hpp"

// One prepared instrument, observed at its graph input just before processing.
// The audio thread uses bounded storage and lock-free atomics only.
namespace repl_notes {
static_assert(std::atomic<uint64_t>::is_always_lock_free);
inline std::atomic<int32_t> target{-1};
inline std::atomic<uint64_t> revision{0};
inline std::array<std::atomic<uint64_t>, 2> published{};

inline void publish(uint64_t low, uint64_t high) noexcept {
    revision.fetch_add(1);
    published[0].store(low);
    published[1].store(high);
    revision.fetch_add(1);
}

inline bool read(std::array<uint64_t, 2>& bits) noexcept {
    for (int attempt = 0; attempt < 8; ++attempt) {
        const auto before = revision.load();
        if (before & 1) continue;
        bits = {published[0].load(), published[1].load()};
        if (revision.load() == before) return true;
    }
    return false;
}

class InputState {
    repl_trace::Input trace;
    uint64_t held[16][16][2]{};
    uint64_t last_low{}, last_high{};
public:
    void process(int32_t instance, const uint32_t* words, size_t count, uint32_t frames = 0) noexcept {
        trace.process(instance, words, count, frames);
        if (instance != target.load()) return;
        if (count == 0) return;
        // UMP message lengths, including utility, SysEx and stream messages.
        constexpr size_t lengths[16]{1,1,1,2,2,4,1,1,2,2,2,3,3,4,4,4};
        for (size_t i = 0; i < count;) {
            const auto word = words[i];
            const auto type = word >> 28;
            const auto length = lengths[type];
            if (length > count - i) break;
            if (type == 2 || type == 4) {
                auto& channel = held[(word >> 24) & 15][(word >> 16) & 15];
                const auto status = (word >> 20) & 15;
                const auto key = (word >> 8) & 127;
                const auto velocity = type == 2 ? word & 127 : words[i + 1] >> 16;
                if (status == 9 && velocity != 0)
                    channel[key / 64] |= uint64_t{1} << (key % 64);
                else if (status == 8 || (status == 9 && velocity == 0))
                    channel[key / 64] &= ~(uint64_t{1} << (key % 64));
                else if (status == 11 && (key == 120 || key == 123))
                    channel[0] = channel[1] = 0;
            }
            i += length;
        }
        uint64_t low{}, high{};
        for (const auto& group : held)
            for (const auto& channel : group) {
                low |= channel[0];
                high |= channel[1];
            }
        if (low != last_low || high != last_high) {
            publish(low, high);
            last_low = low;
            last_high = high;
        }
    }
};
}
