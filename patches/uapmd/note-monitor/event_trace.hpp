#pragma once
#include <array>
#include <atomic>
#include <cstdint>

// Opt-in bounded trace at actual plugin input; no files or allocation on audio thread.
namespace repl_trace {
struct Record {
    std::atomic<uint64_t> frame{};
    std::atomic<uint32_t> message{}, ready{};
    std::atomic<int32_t> instance{-1};
};
inline std::array<Record, 16384> records;
inline std::atomic<uint32_t> generation{0}, count{0}, overflow{0};
inline std::atomic<uint32_t> programChanges{0};
inline std::atomic<uint32_t> sampleRate{48000};
class Input {
    uint32_t seen{};
    uint64_t frame{};
public:
    void process(int32_t instance, const uint32_t* words, size_t size, uint32_t frames) {
        const auto requested = generation.load(std::memory_order_acquire);
        if (!requested) return;
        if (seen != requested) {
            seen = requested;
            frame = 0;
        }
        uint64_t ticks{};
        const auto rate = sampleRate.load(std::memory_order_relaxed);
        constexpr size_t lengths[]{1,1,1,2,2,4,1,1,2,2,2,3,3,4,4,4};
        for (size_t i = 0; i < size;) {
            const auto word = words[i], type = word >> 28;
            const auto length = lengths[type];
            if (length > size - i) break;
            if (type == 0 && ((word >> 16) & 255) == 0x20) ticks += word & 65535;
            if ((type == 2 || type == 4) && ((word >> 20) & 15) == 12)
                programChanges.fetch_add(1, std::memory_order_relaxed);
            if ((type == 2 || type == 4) && (((word >> 20) & 15) == 8 || ((word >> 20) & 15) == 9)) {
                const auto index = count.fetch_add(1, std::memory_order_relaxed);
                if (index < records.size()) {
                    const auto velocity = type == 2 ? word & 127 : words[i + 1] >> 16;
                    const auto on = ((word >> 20) & 15) == 9 && velocity;
                    records[index].frame.store(frame + (ticks * rate + 15625) / 31250, std::memory_order_relaxed);
                    records[index].message.store(((word >> 8) & 127) | (on ? 128 : 0), std::memory_order_relaxed);
                    records[index].instance.store(instance, std::memory_order_relaxed);
                    records[index].ready.store(requested, std::memory_order_release);
                } else overflow.fetch_add(1, std::memory_order_relaxed);
            }
            i += length;
        }
        frame += frames;
    }
};
}
