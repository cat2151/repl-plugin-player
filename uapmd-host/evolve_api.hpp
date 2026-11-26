#pragma once
#include "evolve_dll.hpp"
#include "live_sequence_api.hpp"
#include "../patches/uapmd/evolve-graphs/graphs.hpp"
#include "../patches/uapmd/headless-clap-host/headless_scope.hpp"
#include "patch_into.hpp"
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <memory>
#include <set>
#include <thread>
#include <vector>

namespace repl_evolve {
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
    // One per slot, sized once before any request can attach.
    std::unique_ptr<std::atomic<Box*>[]> slots;
    uint32_t slot_count{};
    explicit Host(uapmd::SequencerEngine& e) : engine(e) {}
    void size(uint32_t count) {
        slots = std::make_unique<std::atomic<Box*>[]>(count);
        slot_count = count;
    }
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
                if (host->closed) { done(user, 0, -1, "evolve is closed"); return; }
                auto* box = new Box{host->engine.prepareTrack()};
                if (!box->track) { delete box; done(user, 0, -1, "Cannot prepare evolve graph"); return; }
                repl_headless::Scope headless; // creation below is synchronous on main
                host->engine.addPluginToPreparedTrack(*box->track, format, plugin,
                    [host, box, done, user](int32_t id, std::string error) {
                        onMain([host, box, done, user, id, error = std::move(error)] {
                            auto* instance = id < 0 ? nullptr : box->track->pluginInstance(id);
                            if (!instance || host->closed) {
                                delete box;
                                done(user, 0, -1, !instance ? error.c_str() : "evolve is closed");
                                return;
                            }
                            box->instance = id;
                            instance->bypassed(true);
                            instance->stopProcessing();
                            host->boxes.insert(box);
                            done(user, reinterpret_cast<uint64_t>(box), id, nullptr);
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
                if (!instance) { done(user, false, "Unknown evolve instance"); return; }
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
        if (error.empty() && !instance) error = "The evolve instance was destroyed";
        else if (error.empty() && instance->startProcessing() != 0) error = "Cannot start evolve plugin";
        else if (error.empty()) instance->bypassed(false);
        done(user, error.empty(), error.empty() ? nullptr : error.c_str());
    }
    static bool attach(void* ctx, uint32_t slot, uint64_t handle) {
        auto* host = static_cast<Host*>(ctx);
        if (slot >= host->slot_count || !handle) return false;
        host->slots[slot].store(reinterpret_cast<Box*>(handle), std::memory_order_release);
        return true;
    }
    static uint64_t detach(void* ctx, uint32_t slot) {
        auto* host = static_cast<Host*>(ctx);
        if (slot >= host->slot_count) return 0;
        return reinterpret_cast<uint64_t>(host->slots[slot].exchange(nullptr, std::memory_order_acq_rel));
    }
};
}

// Unpublished UAPMD graphs supply the plugin ABI only. Rust owns the workers,
// the instance pool, the groups and their slots, clocks, fades and the slot handoff.
class Evolve final : public uapmd::AudioProcessingEventHandler, public repl_evolve_graphs::Source {
    uapmd::SequencerEngine& engine;
    std::shared_ptr<repl_evolve::Host> host;
    repl_evolve::HostApi host_api;
    void* state;
    int32_t first;
    // Whether each slot sounds this block, as `before` returned it.
    std::unique_ptr<std::atomic<bool>[]> enabled;
public:
    struct Group { std::vector<int32_t> tracks; std::string input; };
    Evolve(uapmd::SequencerEngine& e, const std::vector<Group>& groups, double hold, double fade)
        : engine(e), host(std::make_shared<repl_evolve::Host>(e)), host_api(host->api()), state(nullptr), first(INT32_MAX) {
        std::vector<repl_evolve::Group> layout;
        for (const auto& group : groups) {
            layout.push_back({group.tracks.data(), static_cast<uint32_t>(group.tracks.size()), group.input.c_str()});
            for (const auto track : group.tracks) first = std::min(first, track);
        }
        state = repl_evolve::api->create(layout.data(), static_cast<uint32_t>(layout.size()), hold, fade, &host_api);
        if (!state) throw std::runtime_error("evolve needs groups of two distinct tracks with input chord or timeline, "
                                             "at least five seconds hold and positive fade");
        const auto count = repl_evolve::api->slot_count(state);
        host->size(count);
        enabled = std::make_unique<std::atomic<bool>[]>(count);
        engine.addAudioProcessingEventHandler(*this);
        engine.resetTrackProcessingState(first, false, [&] {
            repl_evolve_graphs::active.store(this, std::memory_order_release);
        });
    }
    // Main thread. The Rust destroy waits for `done` callbacks that only the
    // main thread's task queue delivers, so it runs on a helper thread.
    ~Evolve() {
        engine.resetTrackProcessingState(first, false, [&] {
            repl_evolve_graphs::active.store(nullptr, std::memory_order_release);
        });
        engine.removeAudioProcessingEventHandler(*this);
        std::atomic<bool> finished{};
        std::thread closer([&] { repl_evolve::api->destroy(state); finished.store(true); });
        while (!finished.load()) {
            remidy::EventLoop::processQueuedTasks();
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        closer.join();
        remidy::EventLoop::processQueuedTasks();
        host->close();
    }
    bool supportsParallelTrackProcessing() const noexcept override {
        for (uint32_t slot = 0; slot < host->slot_count; ++slot)
            if (auto* box = host->slots[slot].load(std::memory_order_acquire); box && !box->track->track().graph().supportsParallelTrackProcessing())
                return false;
        return true;
    }
    uapmd_graph::AudioPluginGraph* graph(int32_t track) noexcept override {
        const auto slot = repl_evolve::api->slot_of(state, track);
        if (slot < 0 || !enabled[slot].load(std::memory_order_relaxed)) return nullptr;
        auto* box = host->slots[slot].load(std::memory_order_acquire);
        return box ? &box->track->track().graph() : nullptr;
    }
    bool pending() const { return repl_evolve::api->pending(state); }
    void set(const choc::value::ValueView& args) {
        const auto plan = sequence_plan(args, engine);
        const auto selected = args["tracks"];
        if (!selected.isArray()) throw std::runtime_error("Sequence must address the tracks of an evolve chord group");
        std::vector<int32_t> tracks;
        for (uint32_t i = 0; i < selected.size(); ++i) tracks.push_back(selected[i].get<int32_t>());
        const auto result = repl_evolve::api->publish(state, &plan, tracks.data(), static_cast<uint32_t>(tracks.size()));
        if (result == 3) throw std::runtime_error("Sequence must address the tracks of an evolve chord group");
        if (result != 0) throw std::runtime_error("evolve sequence queue is full or invalid");
    }
    uint32_t submit(uint32_t slot, const std::string& format, const std::string& plugin, const std::string& file,
                    const choc::value::ValueView& requested) {
        const auto at = requested.isVoid() ? repl_evolve::current_generation : static_cast<uint64_t>(requested.get<double>());
        const auto id = repl_evolve::api->submit(state, slot, format.c_str(), plugin.c_str(), file.c_str(), at);
        if (!id) throw std::runtime_error("evolve rejected the request or cannot read its state");
        return id;
    }
    uint64_t bumpGeneration() { return repl_evolve::api->bump_generation(state); }
    choc::value::Value poll() { return choc::json::parse(repl_evolve::api->poll(state)); }
    choc::value::Value status() const { return choc::json::parse(repl_evolve::api->status(state)); }
    void beforeTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        const auto slot = repl_evolve::api->slot_of(state, event.track_index);
        if (slot < 0) return;
        enabled[slot].store(false, std::memory_order_relaxed);
        if (event.frame_count <= 0 || event.frame_count > 8192 || !engine.isPlaybackActive()) return;
        auto& context = event.context;
        const auto rate = context.masterContext().sampleRate();
        if (!std::isfinite(rate) || rate <= 0) return;
        auto& input = context.eventIn();
        size_t position = input.position();
        const auto on = repl_evolve::api->before(state, event.track_index, static_cast<uint32_t>(event.frame_count), rate,
            static_cast<uint8_t*>(input.getMessages()), &position, input.maxMessagesInBytes());
        input.position(position);
        enabled[slot].store(on, std::memory_order_relaxed);
    }
    void afterTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        const auto slot = repl_evolve::api->slot_of(state, event.track_index);
        if (slot < 0 || event.frame_count <= 0 || event.frame_count > 8192 || !engine.isPlaybackActive()) return;
        auto& context = event.context;
        const auto frames = static_cast<uint32_t>(event.frame_count);
        // All channels in one call: the limiter links them. Rust silences the ones past its maximum.
        std::array<void*, 64> channels{};
        uint32_t count = 0;
        const bool wide = context.masterContext().audioDataType() == remidy::AudioContentType::Float64;
        if (enabled[slot].load(std::memory_order_relaxed)) {
            for (uint32_t bus = 0; bus < context.audioOutBusCount(); ++bus)
                for (uint32_t channel = 0; channel < context.outputChannelCount(bus); ++channel)
                    if (count < channels.size())
                        channels[count++] = wide ? static_cast<void*>(context.getDoubleOutBuffer(bus, channel))
                                                 : static_cast<void*>(context.getFloatOutBuffer(bus, channel));
        } else context.clearAudioOutputs();
        repl_evolve::api->after(state, event.track_index, channels.data(), count, frames, wide);
    }
};
