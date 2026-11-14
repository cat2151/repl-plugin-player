#include <uapmd-graph/uapmd-graph.hpp>
#include <cmath>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include "routing.hpp"

using namespace uapmd_graph;
void repl_capture_scope(int, remidy::AudioProcessContext&) {}
static void check(bool value, const char* message) {
    if (!value) throw std::runtime_error(message);
}

int main() {
    try {
        check(repl_dag::processingGroup(1,0) == 1 && repl_dag::processingGroup(0xff,2) == 2, "persisted track assignment must precede device group");
        auto graph = AudioPluginFullDAGraph::create(4096);
        graph->getExtension<AudioBusesLayoutExtension>()->applyBusesLayout({1,1,1,1});
        auto gain = [&](const char* id, double value) {
            AudioGraphNodeDescriptor node;
            node.node_id = id;
            node.node_type = "webaudio:GainNode";
            node.parameters.emplace("gain", value);
            check(graph->appendBuiltInNodeSimple(node) == 0, "append gain");
        };
        gain("builtin:track_gain", 0.2);
        gain("layer:0", 0.5);
        gain("layer:1", 0.25);
        gain("unused", 0.1);
        auto* edges = graph->getExtension<GraphConnectionExtension>();
        edges->clearConnections();
        int64_t id = 0;
        auto connect = [&](const char* from, const char* to) {
            auto endpoint = [](const char* name) {
                const std::string value(name);
                return AudioPluginGraphEndpoint{
                    value == "graph:input" ? AudioPluginGraphEndpointType::GraphInput :
                    value == "graph:output" ? AudioPluginGraphEndpointType::GraphOutput : AudioPluginGraphEndpointType::Plugin,
                    value, -1, 0};
            };
            check(edges->connect({++id,AudioPluginGraphBusType::Audio,endpoint(from),endpoint(to)}) == 0, "connect routed gain");
        };
        connect("graph:input", "layer:0");
        connect("graph:input", "layer:1");
        connect("layer:0", "graph:output");
        connect("layer:1", "graph:output");
        remidy::MasterContext master;
        remidy::AudioProcessContext process(master, 4096);
        process.configureMainBus(2,2,8);
        process.frameCount(8);
        for (int channel=0;channel<2;++channel)
            for (int frame=0;frame<8;++frame) process.getFloatInBuffer(0,channel)[frame] = 1.0f;
        check(graph->processAudio(process) == 0, "process graph");
        for (int channel=0;channel<2;++channel)
            for (int frame=0;frame<8;++frame)
                check(std::abs(process.getFloatOutBuffer(0,channel)[frame] - 0.15f) < 1e-6, "parallel gains must sum before track fader, once");
        const uint32_t words[]{0x21902464,0x00400060,0x41902400,0xc8000000};
        std::memcpy(process.eventIn().getMessages(), words, sizeof(words));
        process.eventIn().position(sizeof(words));
        repl_dag::normalizeInstanceGroup(process.eventIn(), 0xff);
        check(std::memcmp(process.eventIn().getMessages(), words, sizeof(words)) == 0, "unassigned group must preserve ports");
        repl_dag::normalizeInstanceGroup(process.eventIn(), 1);
        const auto* remapped = static_cast<const uint32_t*>(process.eventIn().getMessages());
        check(remapped[0] == 0x20902464 && remapped[1] == words[1] && remapped[2] == 0x40902400 && remapped[3] == words[3], "assigned instance group must enter local port zero");
        std::cout << "PASS: routed gains sum once before the implicit fader; disconnected gain has no effect\n";
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
