#pragma once
#include <atomic>
#include <cstdint>
#include <map>
#include <mutex>
#include <optional>

// App-local observation during metadata enumeration, never from the audio path.
namespace repl_parameters {
struct Info { uint32_t stable_id, flags; uint64_t epoch; };
inline std::atomic<uint64_t> next_epoch{1};
inline std::mutex mutex;
inline std::map<const void*, uint64_t> epochs;
inline thread_local int32_t requested = -1;
inline thread_local std::optional<Info> observed;
inline void invalidate(const void* owner) {
    std::lock_guard lock(mutex);
    epochs[owner] = next_epoch++;
}
inline void destroy(const void* owner) {
    std::lock_guard lock(mutex);
    epochs.erase(owner);
}
inline void report(const void* owner, uint32_t index, uint32_t stable_id, uint32_t flags) {
    if (requested < 0 || index != static_cast<uint32_t>(requested)) return;
    std::lock_guard lock(mutex);
    auto [entry, inserted] = epochs.try_emplace(owner, 0);
    if (inserted) entry->second = next_epoch++;
    observed = Info{stable_id, flags, entry->second};
}
struct Capture {
    explicit Capture(int32_t index) { requested = index; observed.reset(); }
    ~Capture() { requested = -1; observed.reset(); }
};
}
