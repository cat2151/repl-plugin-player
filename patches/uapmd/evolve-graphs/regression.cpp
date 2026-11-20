#include "graphs.hpp"
#include <cstdlib>
void require(bool value) { if (!value) std::abort(); }
namespace uapmd_graph { class AudioPluginGraph {}; }
struct Voices final : repl_evolve_graphs::Source {
    uapmd_graph::AudioPluginGraph voice;
    uapmd_graph::AudioPluginGraph* graph(int32_t track) noexcept override { return track==3?&voice:nullptr; }
};
int main() {
    uapmd_graph::AudioPluginGraph ordinary;
    Voices voices;
    require(repl_evolve_graphs::select(3,&ordinary)==&ordinary);
    require(repl_evolve_graphs::workerBudget()==0.8);
    repl_evolve_graphs::active.store(&voices);
    require(repl_evolve_graphs::select(3,&ordinary)==&voices.voice);
    require(repl_evolve_graphs::select(0,&ordinary)==&ordinary);
    require(repl_evolve_graphs::workerBudget()==1.0);
    repl_evolve_graphs::active.store(nullptr);
    require(repl_evolve_graphs::select(3,&ordinary)==&ordinary);
    require(repl_evolve_graphs::workerBudget()==0.8);
}
