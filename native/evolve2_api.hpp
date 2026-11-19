#pragma once
#include "evolve2_dll.hpp"
#include "live_sequence_api.hpp"
#include "../patches/uapmd/evolution-graphs/graphs.hpp"
#include "../patches/uapmd/headless-clap-host/headless_scope.hpp"
#include "patch_into.hpp"
#include <atomic>
#include <fstream>
#include <memory>
#include <set>
#include <thread>

namespace repl_evolve2 {
// One physical instance: an unpublished track holding one plugin. Created,
// read and destroyed on the main thread only; its address is the handle.
struct Box {
    std::unique_ptr<uapmd::PreparedSequencerTrack> track;
    int32_t instance{-1};
    // A box is reloaded with patches of the same plugin.
    bool loaded{};
};

// The HostApi side. Rust workers call it from their threads; every UAPMD call
// is queued to the main thread and `done` is called from there.
class Host final : public std::enable_shared_from_this<Host> {
    uapmd::SequencerEngine& engine;
    std::set<Box*> boxes;
    bool closed{};
public:
    std::array<std::atomic<Box*>, 2> slots{};
    explicit Host(uapmd::SequencerEngine& e) : engine(e) {}
    ~Host() { for (auto* box : boxes) delete box; }
    // Main thread only.
    Box* find(uint64_t handle) const {
        auto* box = reinterpret_cast<Box*>(handle);
        return boxes.count(box) ? box : nullptr;
    }
    // Main thread only; instances created after this are destroyed at once.
    void close() {
        closed = true;
        for (auto* box : boxes) delete box;
        boxes.clear();
    }
    HostApi api() {
        return {sizeof(HostApi), this, create, destroy, load, attach, detach};
    }
private:
    static std::shared_ptr<Host> self(void* ctx) { return static_cast<Host*>(ctx)->shared_from_this(); }
    template<class F> static void onMain(F task) {
        if (remidy::EventLoop::runningOnMainThread()) task();
        else remidy::EventLoop::enqueueTaskOnMainThread(std::move(task));
    }
    static void create(void* ctx, const char* format, const char* plugin, CreateDone done, void* user) {
        remidy::EventLoop::enqueueTaskOnMainThread(
            [host = self(ctx), format = std::string(format), plugin = std::string(plugin), done, user]() mutable {
                if (host->closed) { done(user, 0, "evolve2 is closed"); return; }
                auto* box = new Box{host->engine.prepareTrack()};
                if (!box->track) { delete box; done(user, 0, "Cannot prepare evolve2 graph"); return; }
                repl_headless::Scope headless; // creation below is synchronous on main
                host->engine.addPluginToPreparedTrack(*box->track, format, plugin,
                    [host, box, done, user](int32_t id, std::string error) {
                        onMain([host, box, done, user, id, error = std::move(error)] {
                            auto* instance = id < 0 ? nullptr : box->track->pluginInstance(id);
                            if (!instance || host->closed) {
                                delete box;
                                done(user, 0, !instance ? error.c_str() : "evolve2 is closed");
                                return;
                            }
                            box->instance = id;
                            instance->bypassed(true);
                            instance->stopProcessing();
                            host->boxes.insert(box);
                            done(user, reinterpret_cast<uint64_t>(box), nullptr);
                        });
                    });
            });
    }
    static void destroy(void* ctx, uint64_t handle) {
        remidy::EventLoop::enqueueTaskOnMainThread([host = self(ctx), handle] {
            if (auto* box = host->find(handle)) { host->boxes.erase(box); delete box; }
        });
    }
    static void load(void* ctx, uint64_t handle, const uint8_t* data, size_t size, LoadDone done, void* user) {
        remidy::EventLoop::enqueueTaskOnMainThread(
            [host = self(ctx), handle, bytes = std::vector<uint8_t>(data, data + size), done, user]() mutable {
                auto* box = host->find(handle);
                auto* instance = box ? box->track->pluginInstance(box->instance) : nullptr;
                if (!instance) { done(user, false, "Unknown evolve2 instance"); return; }
                // The slot is claimed and silent, so nothing processes this instance.
                instance->bypassed(true);
                instance->stopProcessing();
                auto patched = repl_patch_into::into(instance, bytes, !box->loaded);
                box->loaded = true;
                if (!patched.error.empty() || patched.state.empty()) {
                    loaded(host, handle, done, user, std::move(patched.error));
                    return;
                }
                instance->loadState(std::move(patched.state), uapmd_plugin_hosting::StateContextType::Project, false, nullptr,
                    [host, handle, done, user](std::string error, void*) {
                        onMain([host, handle, done, user, error = std::move(error)]() mutable {
                            loaded(host, handle, done, user, std::move(error));
                        });
                    });
            });
    }
    // Main thread: restarts the instance after its state was loaded.
    static void loaded(const std::shared_ptr<Host>& host, uint64_t handle, LoadDone done, void* user, std::string error) {
        auto* box = host->find(handle);
        auto* instance = box ? box->track->pluginInstance(box->instance) : nullptr;
        if (error.empty() && !instance) error = "The evolve2 instance was destroyed";
        else if (error.empty() && instance->startProcessing() != 0) error = "Cannot start evolve2 plugin";
        else if (error.empty()) instance->bypassed(false);
        done(user, error.empty(), error.empty() ? nullptr : error.c_str());
    }
    static bool attach(void* ctx, uint32_t slot, uint64_t handle) {
        if (slot >= 2 || !handle) return false;
        static_cast<Host*>(ctx)->slots[slot].store(reinterpret_cast<Box*>(handle), std::memory_order_release);
        return true;
    }
    static uint64_t detach(void* ctx, uint32_t slot) {
        if (slot >= 2) return 0;
        return reinterpret_cast<uint64_t>(static_cast<Host*>(ctx)->slots[slot].exchange(nullptr, std::memory_order_acq_rel));
    }
};
}

// Unpublished UAPMD graphs supply the plugin ABI only. Rust owns the workers,
// the instance pool, clocks, fades and the slot handoff.
class Evolve2 final : public uapmd::AudioProcessingEventHandler, public repl_evolution_graphs::Source {
    uapmd::SequencerEngine& engine;
    std::shared_ptr<repl_evolve2::Host> host;
    repl_evolve2::HostApi host_api;
    void* state;
    std::array<int32_t, 2> tracks;
    std::array<bool, 2> enabled{};
    uint64_t generation{};
public:
    Evolve2(uapmd::SequencerEngine& e, std::array<int32_t, 2> t, double hold, double fade)
        : engine(e), host(std::make_shared<repl_evolve2::Host>(e)), host_api(host->api()),
          state(repl_evolve2::api->create(hold, fade, &host_api)), tracks(t) {
        if (!state) throw std::runtime_error("evolve2 needs at least five seconds hold and positive fade");
        engine.addAudioProcessingEventHandler(*this);
        engine.resetTrackProcessingState(tracks[0], false, [&] {
            repl_evolution_graphs::active.store(this, std::memory_order_release);
        });
    }
    // Main thread. The Rust destroy waits for `done` callbacks that only the
    // main thread's task queue delivers, so it runs on a helper thread.
    ~Evolve2() {
        engine.resetTrackProcessingState(tracks[0], false, [&] {
            repl_evolution_graphs::active.store(nullptr, std::memory_order_release);
        });
        engine.removeAudioProcessingEventHandler(*this);
        std::atomic<bool> finished{};
        std::thread closer([&] { repl_evolve2::api->destroy(state); finished.store(true); });
        while (!finished.load()) {
            remidy::EventLoop::processQueuedTasks();
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        closer.join();
        remidy::EventLoop::processQueuedTasks();
        host->close();
    }
    bool supportsParallelTrackProcessing() const noexcept override {
        for (auto& slot : host->slots)
            if (auto* box = slot.load(std::memory_order_acquire); box && !box->track->track().graph().supportsParallelTrackProcessing())
                return false;
        return true;
    }
    uapmd_graph::AudioPluginGraph* graph(int32_t track) noexcept override {
        for (uint32_t slot = 0; slot < 2; ++slot)
            if (track == tracks[slot] && enabled[slot])
                if (auto* box = host->slots[slot].load(std::memory_order_acquire)) return &box->track->track().graph();
        return nullptr;
    }
    bool pending() const { return repl_evolve2::api->pending(state); }
    void set(const choc::value::ValueView& args) {
        const auto plan = sequence_plan(args, engine);
        const auto selected = args["tracks"];
        if (plan.track != tracks[0] || !selected.isArray() || selected.size() != 2 ||
            selected[0].get<double>() != tracks[0] || selected[1].get<double>() != tracks[1])
            throw std::runtime_error("Sequence must address the two evolve2 tracks");
        if (repl_evolve2::api->publish(state, &plan) != 0)
            throw std::runtime_error("evolve2 sequence queue is full or invalid");
    }
    uint32_t submit(uint32_t slot, const std::string& format, const std::string& plugin, const std::string& file,
                    const choc::value::ValueView& requested) {
        std::ifstream input(std::filesystem::u8path(file), std::ios::binary);
        if (!input) throw std::runtime_error("Cannot read evolve2 state");
        const std::vector<uint8_t> bytes((std::istreambuf_iterator<char>(input)), {});
        const auto at = requested.isVoid() ? generation : static_cast<uint64_t>(requested.get<double>());
        const auto id = repl_evolve2::api->submit(state, slot, format.c_str(), plugin.c_str(), bytes.data(), bytes.size(), at);
        if (!id) throw std::runtime_error("evolve2 rejected the request");
        return id;
    }
    uint64_t bumpGeneration() { return generation = repl_evolve2::api->bump_generation(state); }
    choc::value::Value poll() {
        auto result = choc::value::createEmptyArray();
        std::array<repl_evolve2::Outcome, 16> outcomes;
        for (;;) {
            const auto count = repl_evolve2::api->poll(state, outcomes.data(), static_cast<uint32_t>(outcomes.size()));
            for (uint32_t i = 0; i < count; ++i)
                result.addArrayElement(choc::value::createObject("", "requestId", static_cast<int64_t>(outcomes[i].request_id),
                    "slot", static_cast<int32_t>(outcomes[i].slot), "elapsedMs", outcomes[i].elapsed_ms,
                    "stale", outcomes[i].stale, "error", std::string(outcomes[i].error)));
            if (count < outcomes.size()) return result;
        }
    }
    // Main thread: reads the instance ids out of the main-only boxes.
    choc::value::Value status() const {
        auto result = choc::value::createEmptyArray();
        for (uint32_t slot = 0; slot < 2; ++slot) {
            repl_evolve2::SlotStatus s{};
            repl_evolve2::api->status(state, slot, &s);
            const auto* box = host->find(s.instance);
            result.addArrayElement(choc::value::createObject("", "state", static_cast<int32_t>(s.state), "gain", s.gain,
                "instance", static_cast<double>(s.instance), "instanceId", box ? box->instance : -1,
                "format", std::string(s.format), "pluginId", std::string(s.plugin_id)));
        }
        return result;
    }
    void beforeTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        const auto slot = event.track_index == tracks[0] ? 0 : event.track_index == tracks[1] ? 1 : -1;
        if (slot < 0) return;
        enabled[slot] = false;
        if (event.frame_count <= 0 || event.frame_count > 8192 || !engine.isPlaybackActive()) return;
        auto& context = event.context;
        const auto frames = static_cast<uint32_t>(event.frame_count);
        const auto rate = context.masterContext().sampleRate();
        if (!std::isfinite(rate) || rate <= 0) return;
        if (slot == 0) repl_evolve2::api->begin(state, frames, rate);
        context.eventIn().position(0);
        struct Output { uapmd::AudioProcessContext* context; double rate; uint32_t ticks{}; } output{&context, static_cast<double>(rate)};
        enabled[slot] = repl_evolve2::api->render(state, slot, frames, rate,
            [](void* user, uint32_t offset, uint8_t pitch, bool on, uint16_t velocity) {
                auto& out = *static_cast<Output*>(user);
                auto& input = out.context->eventIn();
                const auto ticks = static_cast<uint32_t>(std::llround(offset * 31250.0 / out.rate));
                const uint32_t words[]{0x00200000u | (ticks - out.ticks),
                    (on ? 0x40900000u : 0x40800000u) | (static_cast<uint32_t>(pitch) << 8), static_cast<uint32_t>(velocity) << 16};
                if (input.position() + sizeof(words) <= input.maxMessagesInBytes()) {
                    std::memcpy(static_cast<uint8_t*>(input.getMessages()) + input.position(), words, sizeof(words));
                    input.position(input.position() + sizeof(words)); out.ticks = ticks;
                }
            }, &output);
    }
    void afterTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        const auto slot = event.track_index == tracks[0] ? 0 : event.track_index == tracks[1] ? 1 : -1;
        if (slot < 0 || event.frame_count <= 0 || event.frame_count > 8192 || !engine.isPlaybackActive()) return;
        auto& context = event.context;
        const auto frames = static_cast<uint32_t>(event.frame_count);
        if (enabled[slot]) {
            for (uint32_t bus = 0; bus < context.audioOutBusCount(); ++bus)
                for (uint32_t channel = 0; channel < context.outputChannelCount(bus); ++channel)
                    if (context.masterContext().audioDataType() == remidy::AudioContentType::Float64)
                        repl_evolve2::api->mix64(state, slot, context.getDoubleOutBuffer(bus, channel), frames);
                    else repl_evolve2::api->mix(state, slot, context.getFloatOutBuffer(bus, channel), frames);
        } else context.clearAudioOutputs();
        repl_evolve2::api->finish(state, slot);
    }
};
