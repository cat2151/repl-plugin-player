#pragma once
#include <algorithm>
#include <atomic>
#include <cstdint>

namespace repl_frozen_loop {
struct Loop {
    bool enabled{};
    int64_t start{}, end{};
};
// The timeline callback hands its one loop read to the frozen tracks of the same callback:
// TimelineFacade::loopRegion() belongs to the model thread and may be freed under a reader.
// Torn values are safe; read() and wrap() accept any range.
inline std::atomic<bool> loopEnabled{};
inline std::atomic<int64_t> loopStart{}, loopEnd{};
inline void publish(bool enabled, int64_t start, int64_t end) {
    loopStart.store(start, std::memory_order_relaxed);
    loopEnd.store(end, std::memory_order_relaxed);
    loopEnabled.store(enabled, std::memory_order_release);
}
inline Loop current() {
    const bool enabled = loopEnabled.load(std::memory_order_acquire);
    const auto start = loopStart.load(std::memory_order_relaxed);
    const auto end = loopEnd.load(std::memory_order_relaxed);
    return {enabled && end > start, start, end};
}

inline int64_t wrap(int64_t position, bool enabled, int64_t start, int64_t end) {
    if (!enabled || end <= start || position < start) return position;
    return start + (position - start) % (end - start);
}

// A callback can cross the endpoint more than once when a loop is shorter than a block.
template<class Read>
void read(int64_t position, int32_t frames, bool enabled, int64_t start, int64_t end, Read&& consume) {
    enabled = enabled && end > start;
    int32_t offset = 0;
    position = wrap(position, enabled, start, end);
    while (offset < frames) {
        auto count = frames - offset;
        if (enabled) count = static_cast<int32_t>(std::min<int64_t>(count, end - position));
        consume(position, offset, count);
        offset += count;
        position += count;
        if (enabled && position == end) position = start;
    }
}
}
