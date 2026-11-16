#pragma once
#include "sequence_dll.hpp"
#include <atomic>
#include <algorithm>
#include <cmath>

inline repl_sequence::Plan sequence_plan(const choc::value::ValueView& args, uapmd::SequencerEngine& engine) {
    repl_sequence::Plan plan;
    plan.track = args["track"].get<int32_t>();
    const auto bpm = args["bpm"].get<double>();
    const auto beats = args["beats"].get<double>();
    plan.gate = args["gate"].get<double>();
    const auto velocity = args["velocity"].get<int32_t>();
    if (args["track"].get<double>() != plan.track || args["velocity"].get<double>() != velocity ||
        plan.track < 0 || static_cast<size_t>(plan.track) >= engine.tracks().size() ||
        !std::isfinite(bpm) || bpm <= 0 || !std::isfinite(beats) || beats <= 0 ||
        !std::isfinite(plan.gate) || plan.gate <= 0 || plan.gate > 1 || velocity < 1 || velocity > 65535)
        throw std::runtime_error("Invalid live sequence settings");
    plan.seconds = beats * 60 / bpm;
    plan.velocity = static_cast<uint16_t>(velocity);
    const auto steps = args["steps"];
    if (!steps.isArray() || steps.size() < 1 || steps.size() > plan.steps.size())
        throw std::runtime_error("Expected 1..64 sequence steps");
    for (const auto& notes : steps) {
        if (!notes.isArray() || notes.size() < 1 || notes.size() > 8)
        throw std::runtime_error("Expected 1..8 pitches per step");
        auto& step = plan.steps[plan.count++];
        for (const auto& note : notes) {
        const auto pitch = note.get<int32_t>();
        if (pitch < 0 || pitch > 127 || note.get<double>() != pitch)
            throw std::runtime_error("Invalid MIDI pitch");
        step.notes[step.count++] = static_cast<uint8_t>(pitch);
        }
    }
    return plan;
}

// UAPMD adapter only; the clock and pattern handoff live in Rust sequence.dll.
class LiveSequence final : public uapmd::AudioProcessingEventHandler,
                           public uapmd::SequencerProcessingLifecycleListener {
    uapmd::SequencerEngine& engine;
    repl_sequence::Clock clock;
    bool playing{};
    std::atomic<int32_t> target{-1};
    std::atomic<int32_t> source_instance{-1};
public:
    explicit LiveSequence(uapmd::SequencerEngine& value) : engine(value), playing(value.isPlaybackActive()) {
        engine.addAudioProcessingEventHandler(*this);
        engine.addProcessingLifecycleListener(*this);
    }
    ~LiveSequence() {
        engine.removeAudioProcessingEventHandler(*this);
        engine.removeProcessingLifecycleListener(*this);
    }
    bool supportsParallelTrackProcessing() const noexcept override { return true; }
    bool pending() const { return clock.pending(); }
    void transportTransition(uapmd::SequencerTransportTransition transition, int64_t) override {
        using T = uapmd::SequencerTransportTransition;
        if (transition == T::Started) { clock.rewind(); playing = true; }
        if (transition == T::Stopped || transition == T::Paused) { playing = false; }
        if (transition == T::Resumed) playing = true;
    }
    void processingStateReset() override { clock.rewind(); }
    void synthReset(int32_t index) {
        if (index == target.load()) clock.release();
    }
    void trackRemoved(int32_t index) override {
        if (index > target.load()) return;
        const auto held = clock.heldNotes();
        for (uint8_t i = 0; i < held.count; ++i) engine.sendNoteOff(source_instance, held.notes[i]);
        clock.reset(); target = -1;
    }
    void clear() {
        // This public transition excludes in-flight callbacks, including parallel jobs.
        const auto track = target.load();
        if (track < 0) return;
        repl_sequence::Step held;
        engine.resetTrackProcessingState(track, false, [&] { held = clock.heldNotes(); clock.reset(); target = -1; });
        if (track >= 0 && static_cast<size_t>(track) < engine.tracks().size())
            for (const auto id : engine.tracks()[track]->orderedInstanceIds())
                for (uint8_t i = 0; i < held.count; ++i) engine.sendNoteOff(id, held.notes[i]);
    }
    void set(const choc::value::ValueView& args) {
        const auto plan = sequence_plan(args, engine);
        if (target >= 0 && target != plan.track) throw std::runtime_error("Clear the sequence before changing track");
        const auto& ids = engine.tracks()[plan.track]->orderedInstanceIds();
        if (ids.empty()) throw std::runtime_error("Sequence track needs an instrument");
        auto source = ids.front();
        auto& graph = engine.tracks()[plan.track]->graph();
        if (auto* connections = graph.getExtension<uapmd_graph::GraphConnectionExtension>()) {
            for (const auto& connection : connections->connections()) {
                if (connection.bus_type != uapmd_graph::AudioPluginGraphBusType::Event ||
                    connection.source.type != uapmd_graph::AudioPluginGraphEndpointType::GraphInput ||
                    connection.target.type != uapmd_graph::AudioPluginGraphEndpointType::Plugin) continue;
                source = connection.target.instance_id;
                if (!connection.target.node_id.empty())
                    for (const auto& [id, node] : graph.plugins())
                        if (node->nodeId() == connection.target.node_id) { source = id; break; }
                break;
            }
        }
        if (!clock.publish(plan)) throw std::runtime_error("Sequence update queue full; wait for the next step");
        source_instance = source;
        target = plan.track;
    }
    void beforeTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        if (!playing || target < 0 || event.track_index != target || event.frame_count <= 0) return;
        auto& input = event.context.eventIn();
        // This experimental source owns its track's event input, not its audio graph.
        input.position(0);
        uint32_t previousTicks{};
        const auto rate = event.context.masterContext().sampleRate();
        if (!(std::isfinite(rate) && rate > 0)) return;
        const auto source = source_instance.load();
        const auto assigned = source < 0 ? 0 : event.track.processingGroup(source);
        const uint32_t group = static_cast<uint32_t>(assigned <= 15 ? assigned : 0) << 24;
        auto* instance = source < 0 ? nullptr : engine.getPluginInstance(source);
        const bool enabled = instance && !instance->bypassed();
        clock.render(event.track_index, static_cast<uint32_t>(event.frame_count), rate, enabled,
            [&](uint32_t offset, uint8_t pitch, bool on, uint16_t velocity) {
                const auto ticks = static_cast<uint32_t>(std::llround(offset * 31250.0 / rate));
                const uint32_t words[]{0x00200000u | (ticks - previousTicks),
                    (on ? 0x40900000u : 0x40800000u) | group | (static_cast<uint32_t>(pitch) << 8),
                    static_cast<uint32_t>(velocity) << 16};
                if (input.position() + sizeof(words) <= input.maxMessagesInBytes()) {
                    std::memcpy(static_cast<uint8_t*>(input.getMessages()) + input.position(), words, sizeof(words));
                    input.position(input.position() + sizeof(words));
                    previousTicks = ticks;
                }
            });
    }
};
