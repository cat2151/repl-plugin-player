#pragma once
#include <array>
#include <atomic>
#include <cstdint>
#include <cstddef>
#include "event_trace.hpp"

// Every plugin node, observed at its graph input just before processing.
// Each node publishes its held keys to its own slot; readers take the union.
// The audio thread uses bounded storage and lock-free atomics only.
namespace repl_notes {
static_assert(std::atomic<uint64_t>::is_always_lock_free);
// The prepared instrument; only the scope output uses it.
inline std::atomic<int32_t> target{-1};
// Nodes beyond this count are not shown on the keyboard.
inline constexpr size_t slot_count = 64;
inline std::array<std::atomic<bool>, slot_count> claimed{};
inline std::array<std::array<std::atomic<uint64_t>, 2>, slot_count> published{};

inline std::array<uint64_t, 2> read() noexcept {
    std::array<uint64_t, 2> bits{};
    for (const auto& slot : published) {
        bits[0] |= slot[0].load();
        bits[1] |= slot[1].load();
    }
    return bits;
}

class InputState {
    repl_trace::Input trace;
    uint64_t held[16][16][2]{};
    uint64_t last_low{}, last_high{};
    int slot = -1;
public:
    InputState() noexcept {
        for (size_t i = 0; i < slot_count && slot < 0; ++i) {
            bool free = false;
            if (claimed[i].compare_exchange_strong(free, true)) slot = static_cast<int>(i);
        }
    }
    ~InputState() {
        if (slot < 0) return;
        published[slot][0].store(0);
        published[slot][1].store(0);
        claimed[slot].store(false);
    }
    InputState(const InputState&) = delete;
    InputState& operator=(const InputState&) = delete;
    void process(int32_t instance, const uint32_t* words, size_t count, uint32_t frames = 0) noexcept {
        trace.process(instance, words, count, frames);
        if (slot < 0 || count == 0) return;
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
            published[slot][0].store(low);
            published[slot][1].store(high);
            last_low = low;
            last_high = high;
        }
    }
};
}
