#pragma once
#include <atomic>
#include <cstdint>
namespace uapmd_graph { class AudioPluginGraph; }
namespace repl_evolve_graphs {
// Only a registered evolve adapter redirects graphs. Registration/removal uses
// the engine's render exclusion; callbacks and graph selection are serialized.
struct Source {
    virtual ~Source() = default;
    virtual uapmd_graph::AudioPluginGraph* graph(int32_t track) noexcept = 0;
};
inline std::atomic<Source*> active{};
inline uapmd_graph::AudioPluginGraph* select(int32_t track,uapmd_graph::AudioPluginGraph* ordinary) noexcept {
    auto* source=active.load(std::memory_order_acquire);
    auto* graph=source?source->graph(track):nullptr;
    return graph?graph:ordinary;
}
inline double workerBudget() noexcept {
    // evolve after-callbacks only multiply gains; DSP belongs to worker jobs.
    // Use the actual block deadline, rather than the ordinary 80% early cutoff.
    return active.load(std::memory_order_acquire)?1.0:0.8;
}
}
