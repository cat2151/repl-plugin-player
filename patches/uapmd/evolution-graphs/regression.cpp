#include "graphs.hpp"
#include <cstdlib>
void require(bool value) { if (!value) std::abort(); }
int polled{};
void poll() { ++polled; }
namespace uapmd_graph { class AudioPluginGraph {}; }
struct Voices final : repl_evolution_graphs::Source {
    uapmd_graph::AudioPluginGraph voice;
    uapmd_graph::AudioPluginGraph* graph(int32_t track) noexcept override { return track==3?&voice:nullptr; }
};
int main() {
    repl_evolution_graphs::pollWorkers();
    require(polled==0);
    repl_evolution_graphs::worker_poll.store(poll);
    repl_evolution_graphs::pollWorkers();
    require(polled==1);
    repl_evolution_graphs::worker_poll.store(nullptr);
    uapmd_graph::AudioPluginGraph ordinary;
    Voices voices;
    require(repl_evolution_graphs::select(3,&ordinary)==&ordinary);
    require(repl_evolution_graphs::workerBudget()==0.8);
    repl_evolution_graphs::active.store(&voices);
    require(repl_evolution_graphs::select(3,&ordinary)==&voices.voice);
    require(repl_evolution_graphs::select(0,&ordinary)==&ordinary);
    require(repl_evolution_graphs::workerBudget()==1.0);
    repl_evolution_graphs::active.store(nullptr);
    require(repl_evolution_graphs::select(3,&ordinary)==&ordinary);
    require(repl_evolution_graphs::workerBudget()==0.8);
}
