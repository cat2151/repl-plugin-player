#include <uapmd-app-model/uapmd-app-model.hpp>
#include <choc/text/choc_JSON.h>
#include <chrono>
#include <mutex>
#include <queue>
#include <thread>
#include <stdexcept>
#include <cstdio>
#include <cmath>
#include <array>
#include "../patches/uapmd/note-monitor/note_monitor.hpp"
#include "../patches/uapmd/catalog-cache-startup/catalog_cache.hpp"
#include "../patches/uapmd/clap-catalog-complete/clap_catalog.hpp"
#include "localization.hpp"
#include "plugin_windows.hpp"
#include "parameter_safety.hpp"
#include "timeline_api.hpp"
#include "timeline_view.hpp"
#include "piano_roll_api.hpp"
#include "project_api.hpp"
#include "scope_capture.hpp"
#include "freeze_transport.hpp"
#include "live_sequence_api.hpp"
#include "evolve_api.hpp"
#include "fresh_instance.hpp"
#include "../patches/uapmd/clap-plugin-handle/clap_plugin_handle.hpp"
#include "../patches/uapmd/clap-sysex-input/clap_sysex_input.hpp"

using ScopeCallback = void (*)(void*, const void*, const void*, size_t, uint32_t, const uint32_t*, size_t);
static ScopeCallback scope_callback{};
static void* scope_context{};
using PadGain = void (*)(int32_t, void* const*, size_t, size_t, double, bool, const uint32_t*, size_t);
static PadGain pad_gain{};
// The device callback's context keeps its default sample rate on MiniAudio.
static std::atomic<double> pad_gain_rate{};
extern "C" __declspec(dllexport) void repl_set_pad_gain_callback(PadGain callback) {
    pad_gain = callback;
}
extern "C" __declspec(dllexport) void repl_set_scope(ScopeCallback callback, void* context) {
    scope_callback = callback;
    scope_context = context;
}
void repl_capture_scope(int32_t instance, uapmd::AudioProcessContext& process) noexcept {
    const bool master = instance == -2;
    const bool capture = scope_callback && instance == repl_notes::target.load(std::memory_order_relaxed);
    if ((!capture && !pad_gain) || process.frameCount() <= 0 ||
        process.audioOutBusCount() == 0 || process.outputChannelCount(0) == 0) return;
    const bool doubles = process.masterContext().audioDataType() == remidy::AudioContentType::Float64;
    const auto channels = process.outputChannelCount(0);
    const auto* events = master ? nullptr : static_cast<const uint32_t*>(process.eventIn().getMessages());
    const size_t words = master ? 0 : process.eventIn().position() / sizeof(uint32_t);
    const auto gain = [&] {
        std::array<void*, 16> output;
        if (!pad_gain || channels > output.size()) return;
        for (size_t i = 0; i < channels; ++i)
            output[i] = doubles ? static_cast<void*>(process.getDoubleOutBuffer(0, i))
                                : process.getFloatOutBuffer(0, i);
        pad_gain(instance, output.data(), channels, process.frameCount(),
            pad_gain_rate.load(std::memory_order_relaxed), doubles, events, words);
    };
    if (master) gain();
    if (capture) {
        const void* left = doubles ? static_cast<const void*>(process.getDoubleOutBuffer(0, 0)) : process.getFloatOutBuffer(0, 0);
        const void* right = channels > 1
            ? (doubles ? static_cast<const void*>(process.getDoubleOutBuffer(0, 1)) : process.getFloatOutBuffer(0, 1)) : left;
        if (left && right)
            scope_callback(scope_context, left, right, static_cast<size_t>(process.frameCount()),
                (doubles ? 1u : 0u) | (master ? 2u : 0u), events, words);
    }
    if (!master) gain();
}

using ScriptApi = const char* (*)(const char*);
static ScriptApi script_api{};
static const char* control_api(const char* request);
static bool session_open();
static const char* sequence_api(const char* request);
static const char* evolve_api(const char* request);
extern "C" const char* repl_localized(const char* key) {
    static thread_local std::string value;
    value = key; // Standalone native harnesses register the same catalog callback.
    if (script_api) {
        auto request = choc::value::createObject("", "api", "text", "key", key);
        auto response = choc::json::parse(script_api(choc::json::toString(request).c_str()));
        if (response["ok"].getWithDefault<bool>(false)) value = response["value"].get<std::string>();
    }
    return value.c_str();
}
// The track instance's `clap_plugin*`, or null when it is missing or not CLAP.
// Main/control thread only; valid until the instance is disposed.
extern "C" __declspec(dllexport) const void* repl_clap_plugin(int32_t instanceId) noexcept {
    auto* instance = uapmd_app::AppModel::instance().sequencer().engine()->getPluginInstance(instanceId);
    return instance ? repl_clap_plugin_handle::of(instance) : nullptr;
}
// Queues a MIDI SysEx message (copied) for the next process() of the live CLAP
// instance whose `clap_plugin*` is `plugin`.
extern "C" __declspec(dllexport) void repl_clap_sysex(const void* plugin, const uint8_t* bytes, size_t size) noexcept {
    repl_clap_sysex_input::send(plugin, bytes, size);
}
// Calls `use` with the `clap_plugin*` of a new instance no track holds (null
// when it is not CLAP), then destroys the instance. Main/control thread only.
extern "C" __declspec(dllexport) bool repl_fresh_plugin(const char* format, const char* plugin, repl_fresh_instance::Use use,
                                                        void* user, char* error, size_t capacity) noexcept {
    std::string failed;
    try {
        failed = repl_fresh_instance::with(*uapmd_app::AppModel::instance().sequencer().engine(), format, plugin, use, user);
    } catch (const std::exception& e) {
        failed = e.what();
    }
    if (failed.empty()) return true;
    if (capacity) {
        const auto size = std::min(failed.size(), capacity - 1);
        std::copy_n(failed.data(), size, error);
        error[size] = 0;
    }
    return false;
}
extern "C" __declspec(dllexport) void repl_set_patch_into(repl_patch_into::Into into) {
    repl_patch_into::function = into;
}
extern "C" __declspec(dllexport) void repl_set_script_api(ScriptApi callback) {
    script_api = callback;
}
// Called synchronously on the control thread; copy the Rust-owned reply immediately.
extern "C" const char* repl_script_api(const char* request) {
    if (auto result = evolve_api(request)) return result;
    if (auto result = sequence_api(request)) return result;
    if (auto result = timeline_api(request, session_open())) return result;
    if (auto result = control_api(request)) return result;
    if (auto result = piano_roll_api(request, session_open())) return result;
    if (auto result = project_api(request, session_open())) return result;
    if (!script_api) return "{\"ok\":false,\"error\":\"Script API is not registered\"}";
    return script_api(request);
}

namespace {
class ControlLoop final : public remidy::EventLoop {
    std::thread::id owner{std::this_thread::get_id()};
    std::mutex mutex;
    std::queue<std::function<void()>> tasks;
    void initializeOnUIThreadImpl() override {}
    bool runningOnMainThreadImpl() override { return owner == std::this_thread::get_id(); }
    void enqueueTaskOnMainThreadImpl(std::function<void()>&& task) override {
        std::lock_guard lock(mutex);
        tasks.push(std::move(task));
    }
    void startImpl() override {}
    void stopImpl() override {}
    void processQueuedTasksImpl() override {
        std::queue<std::function<void()>> pending;
        { std::lock_guard lock(mutex); pending.swap(tasks); }
        while (!pending.empty()) {
            auto task = std::move(pending.front());
            pending.pop();
            task();
        }
    }
};
struct Session {
    TimelineView timeline_view;
    ControlLoop loop;
    remidy::EventLoop* previous{};
    bool model_created{};
    bool scanning_done{};
    std::string scan_error;
    // The catalog came from the plugin list cache; the startup scan waits for "refresh".
    bool catalog_from_cache{};
    bool state_pending{};
    bool state_ready{};
    std::string preparation_error;
    std::vector<uint8_t> state;
    std::unique_ptr<uapmd_app::UapmdJSRuntime> runtime;
    std::array<std::unique_ptr<LiveSequence>, 8> sequences;
    std::unique_ptr<Evolve> evolve;
    PluginWindows windows;
    Session(const std::string& data) {
        previous = remidy::getEventLoop();
        remidy::setEventLoop(&loop);
        remidy::EventLoop::initializeOnUIThread();
        uapmd_plugin_hosting::applicationDataDirectory(std::filesystem::u8path(data));
    }
    void initialize() {
        uapmd_app::AppModel::instantiate();
        model_created = true;
        runtime = std::make_unique<uapmd_app::UapmdJSRuntime>();
        runtime->ensureApiBootstrapped();
        auto& model = uapmd_app::AppModel::instance();
        timeline_view.captureInitial(*model.sequencer().engine());
        windows.attach();
        // Runs on the scan worker, before scanning_done lets scripts read the catalog.
        model.scanningCompleted.push_back([this](bool ok, std::string error) {
            if (ok) {
                try {
                    auto* host = uapmd_app::AppModel::instance().sequencer().engine()->pluginHost();
                    if (repl_clap_catalog::complete(*host))
                        host->reloadPluginCatalogFromCache();
                } catch (const std::exception& e) {
                    ok = false;
                    error = std::string("clap-catalog-complete: ") + e.what();
                }
            }
            remidy::EventLoop::enqueueTaskOnMainThread([this, ok, error] {
                scanning_done = true;
                scan_error = ok ? "" : error;
            });
        });
        catalog_from_cache = repl_catalog_cache::load(*model.sequencer().engine()->pluginHost());
        if (catalog_from_cache)
            scanning_done = true;
        else
            startInitialScan();
    }
    // UAPMD's own startup scan: fast formats are enumerated, slow ones come from the cache.
    void startInitialScan() {
        auto& model = uapmd_app::AppModel::instance();
        scanning_done = false;
        scan_error.clear();
        model.notifyUiReady();
        model.notifyPersistentStorageReady();
    }
    void requestState(int32_t target) {
        if (state_pending) throw std::runtime_error(repl_localized("native_00"));
        auto* instance = uapmd_app::AppModel::instance().sequencer().engine()->getPluginInstance(target);
        if (!instance) throw std::runtime_error(repl_localized("native_01"));
        state_ready = false;
        state_pending = true;
        instance->requestState(uapmd_plugin_hosting::StateContextType::Project, false, nullptr,
            [this](std::vector<uint8_t> bytes, std::string error, void*) {
                remidy::EventLoop::enqueueTaskOnMainThread([this, bytes = std::move(bytes), error]() mutable {
                    state = std::move(bytes);
                    preparation_error = error;
                    state_ready = error.empty();
                    state_pending = false;
                });
            });
    }
    std::string stateHex() const {
        std::string hex;
        constexpr char digits[] = "0123456789abcdef";
        for (auto byte : state) { hex += digits[byte >> 4]; hex += digits[byte & 15]; }
        return hex;
    }
    // requestState, with this thread's queue serviced for up to five seconds.
    std::string readState(int32_t target) {
        requestState(target);
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (state_pending && std::chrono::steady_clock::now() < deadline) {
            remidy::EventLoop::processQueuedTasks();
            std::this_thread::sleep_for(std::chrono::milliseconds(5));
        }
        if (!state_pending && !preparation_error.empty()) throw std::runtime_error(preparation_error);
        if (!state_ready) throw std::runtime_error(repl_localized("native_06"));
        return stateHex();
    }
    ~Session() {
        if (model_created) {
            auto& model = uapmd_app::AppModel::instance();
            model.stopPluginScanning();
            while (state_pending || model.historyState().busy) {
                remidy::EventLoop::processQueuedTasks();
                std::this_thread::sleep_for(std::chrono::milliseconds(5));
            }
            model.setAudioEngineEnabled(false);
            while (model.sequencer().isAudioPlaying()) {
                remidy::EventLoop::processQueuedTasks();
                std::this_thread::sleep_for(std::chrono::milliseconds(5));
            }
            remidy::EventLoop::processQueuedTasks();
            windows.closeAll();
            evolve.reset();
            for (auto& sequence : sequences) sequence.reset();
            if (runtime) {
                runtime->unregisterAllParameterListeners();
                runtime->unregisterAllMetadataListeners();
                runtime.reset();
            }
            uapmd_app::AppModel::cleanupInstance();
            remidy::EventLoop::processQueuedTasks();
        }
        remidy::setEventLoop(previous);
    }
};
std::unique_ptr<Session> session;
std::string reply;
}
static bool session_open() { return session != nullptr; }

// The groups of `evolve.open`, on empty tracks that only evolve plays. evolve checks the rest.
static std::vector<Evolve::Group> evolve_groups(const choc::value::ValueView& selected, uapmd::SequencerEngine& engine) {
    if (!selected.isArray()) throw std::runtime_error("Expected evolve groups");
    std::vector<Evolve::Group> groups;
    for (uint32_t g=0;g<selected.size();++g) {
        const auto tracks=selected[g]["tracks"];
        const auto input=selected[g]["input"];
        if (!tracks.isArray() || !input.isString()) throw std::runtime_error("Expected evolve groups of tracks and input");
        Evolve::Group group{{},input.get<std::string>()};
        for (uint32_t i=0;i<tracks.size();++i) {
            const auto track=tracks[i].get<int32_t>();
            if (tracks[i].get<double>()!=track || track<0 || static_cast<size_t>(track)>=engine.tracks().size() ||
                !engine.tracks()[track]->orderedInstanceIds().empty())
                throw std::runtime_error("evolve requires empty dedicated tracks");
            group.tracks.push_back(track);
        }
        groups.push_back(std::move(group));
    }
    return groups;
}

static const char* evolve_api(const char* request) {
    static thread_local std::string reply;
    try {
        const auto args=choc::json::parse(request);
        const auto api=args["api"].get<std::string>();
        if (api.rfind("evolve.",0)!=0) return nullptr;
        if (!session) throw std::runtime_error("No session");
        auto value=choc::value::Value();
        if (api=="evolve.open") {
            if (session->evolve) throw std::runtime_error("evolve is already open");
            if (!repl_evolve::api) throw std::runtime_error("evolve DLL is not loaded");
            auto& engine=*uapmd_app::AppModel::instance().sequencer().engine();
            const auto groups=evolve_groups(args["groups"],engine);
            session->evolve=std::make_unique<Evolve>(engine,groups,args["holdMs"].get<double>()/1000,args["fadeMs"].get<double>()/1000);
        } else if (api=="evolve.close") {
            session->evolve.reset();
        } else {
            if (!session->evolve) throw std::runtime_error("evolve is not open");
            auto& current=*session->evolve;
            if (api=="evolve.status") value=current.status();
            else if (api=="evolve.poll") value=current.poll();
            else if (api=="evolve.bumpGeneration") value=choc::value::createFloat64(static_cast<double>(current.bumpGeneration()));
            else if (api=="evolve.submit") {
                const auto slot=args["slot"].get<int32_t>();
                if (slot<0 || args["slot"].get<double>()!=slot) throw std::runtime_error("Invalid evolve slot");
                value=choc::value::createInt64(current.submit(slot,args["format"].get<std::string>(),args["pluginId"].get<std::string>(),
                    args["file"].get<std::string>(),args["generation"]));
            } else throw std::runtime_error("Unknown evolve API: "+api);
        }
        reply=choc::json::toString(choc::value::createObject("","ok",true,"value",value));
    } catch (const std::exception& error) {
        reply=choc::json::toString(choc::value::createObject("","ok",false,"error",error.what()));
    }
    return reply.c_str();
}

static const char* sequence_api(const char* request) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (api != "sequence.set" && api != "sequence.clear" && api != "sequence.pending") return nullptr;
        if (!session) throw std::runtime_error("No session");
        if (session->evolve) {
            if (api == "sequence.set") session->evolve->set(args);
            const auto value = api == "sequence.pending" ? choc::value::createBool(session->evolve->pending()) : choc::value::Value();
            result = choc::json::toString(choc::value::createObject("", "ok",true,"value",value));
            return result.c_str();
        }
        if (api == "sequence.set") {
            const auto tracks = args["tracks"];
            if (!tracks.isVoid() && (!tracks.isArray() || tracks.size() < 1 || tracks.size() > session->sequences.size()))
                throw std::runtime_error("Expected one to eight distinct sequence tracks");
            const auto count = tracks.isArray() ? tracks.size() : 1u;
            for (uint32_t i = 0; i < count; ++i)
                for (uint32_t j = 0; j < i; ++j)
                    if (tracks[i] == tracks[j]) throw std::runtime_error("Expected distinct sequence tracks");
            for (uint32_t i = 0; i < session->sequences.size(); ++i) {
                if (i < count) {
                    if (!session->sequences[i]) session->sequences[i] = std::make_unique<LiveSequence>(
                        *uapmd_app::AppModel::instance().sequencer().engine());
                    auto plan = args;
                    if (tracks.isArray()) plan.setMember("track", tracks[i]);
                    session->sequences[i]->set(plan);
                } else if (session->sequences[i]) session->sequences[i]->clear();
            }
        }
        else if (api == "sequence.clear") {
            for (auto& sequence : session->sequences) if (sequence) sequence->clear();
        }
        const auto pending = std::any_of(session->sequences.begin(), session->sequences.end(),
            [](const auto& sequence) { return sequence && sequence->pending(); });
        result = choc::json::toString(choc::value::createObject("", "ok", true, "value",
            api == "sequence.pending" ? choc::value::createBool(pending) : choc::value::Value()));
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}

// The master output's latest 256 samples (before soft clip / device mute) and the engine counters.
static void observe_into(choc::value::Value& result) {
    auto& model = uapmd_app::AppModel::instance();
    auto* engine = model.sequencer().engine();
    engine->audioPerformanceCounter().setEnabled(true);
    const auto counters = engine->audioPerformanceCounter().counters();
    uapmd::AudioProcessingTiming timing;
    auto processingTimings = choc::value::createEmptyArray();
    while (engine->audioPerformanceCounter().tryDequeue(timing)) {
        if (timing.offline) continue;
        processingTimings.addArrayElement(choc::value::createObject("", "stage", static_cast<int32_t>(timing.stage),
            "track", timing.track_index, "durationMs", static_cast<double>(timing.duration_nanoseconds) / 1e6,
            "frames", timing.frame_count, "sampleRate", timing.sample_rate,
            "block", static_cast<int64_t>(timing.block_number)));
    }
    result.setMember("processingTimings", processingTimings);
    result.setMember("deadlineMisses", static_cast<int64_t>(counters.deadline_misses));
    result.setMember("droppedTimings", static_cast<int64_t>(counters.dropped_records));
    std::array<float, 256> samples{};
    auto* analyser = engine->outputAnalyser();
    if (!analyser) throw std::runtime_error(repl_localized("native_08"));
    analyser->getFloatTimeDomainData(samples.data(), static_cast<uint32_t>(samples.size()));
    double peak{}, energy{};
    bool finite = true;
    for (float value : samples) {
        finite &= std::isfinite(value);
        peak = std::max(peak, std::abs(static_cast<double>(value)));
        energy += static_cast<double>(value) * value;
    }
    result.setMember("finite", finite);
    result.setMember("peak", finite ? peak : 0.0);
    result.setMember("rms", finite ? std::sqrt(energy / samples.size()) : 0.0);
    result.setMember("blocks", static_cast<int64_t>(counters.realtime_blocks));
    result.setMember("playbackPositionSamples", engine->playbackPosition());
    result.setMember("renderPositionSamples", engine->renderPlaybackPosition());
    result.setMember("engineEnabled", model.isAudioEngineEnabled());
    result.setMember("dispatcherPlaying", model.sequencer().isAudioPlaying() != 0);
    result.setMember("observationPoint", "master graph before soft clip / device mute; history snapshot");
    result.setMember("samples", static_cast<int64_t>(samples.size()));
}

// Pitches the scope target's plugin input holds; a stopped device has no live input.
static choc::value::Value held_notes() {
    auto notes = choc::value::createEmptyArray();
    if (uapmd_app::AppModel::instance().sequencer().isAudioPlaying()) {
        const auto bits = repl_notes::read();
        for (int pitch = 0; pitch < 128; ++pitch)
            if (bits[pitch / 64] & (uint64_t{1} << (pitch % 64)))
                notes.addArrayElement(pitch);
    }
    return notes;
}

// Plugin-input Note On/Off records since the last start, and each track's render lead.
// `start` clears the records; the device must be stopped.
static void loop_trace_into(choc::value::Value& result, bool start) {
    if (start) {
        if (uapmd_app::AppModel::instance().sequencer().isAudioPlaying())
            throw std::runtime_error("Stop audio before starting input trace");
        repl_trace::sampleRate.store(static_cast<uint32_t>(
            uapmd_app::AppModel::instance().sequencer().engine()->currentSampleRate()));
        repl_trace::count.store(0);
        repl_trace::overflow.store(0);
        repl_trace::programChanges.store(0);
        repl_trace::generation.fetch_add(1, std::memory_order_release);
    }
    auto events = choc::value::createEmptyArray();
    const auto count = std::min<uint32_t>(repl_trace::count.load(), static_cast<uint32_t>(repl_trace::records.size()));
    for (uint32_t i = 0; i < count; ++i) {
        if (repl_trace::records[i].ready.load(std::memory_order_acquire) != repl_trace::generation.load()) continue;
        const auto message = repl_trace::records[i].message.load();
        events.addArrayElement(choc::value::createObject("", "frame", static_cast<int64_t>(repl_trace::records[i].frame.load()),
            "note", static_cast<int32_t>(message & 127), "on", (message & 128) != 0,
            "instance", repl_trace::records[i].instance.load()));
    }
    result.setMember("events", events);
    result.setMember("overflow", static_cast<int64_t>(repl_trace::overflow.load()));
    result.setMember("programChanges", static_cast<int64_t>(repl_trace::programChanges.load()));
    auto timing = choc::value::createEmptyArray();
    auto& engine = *uapmd_app::AppModel::instance().sequencer().engine();
    for (size_t i = 0; i < engine.tracks().size(); ++i) {
        auto instances = choc::value::createEmptyArray();
        for (auto id : engine.tracks()[i]->orderedInstanceIds()) instances.addArrayElement(id);
        timing.addArrayElement(choc::value::createObject("", "trackIndex", static_cast<int32_t>(i),
            "lead", static_cast<int64_t>(engine.trackRenderLeadInSamples(static_cast<int32_t>(i)) + engine.masterTrackRenderLeadInSamples()),
            "instances", instances));
    }
    result.setMember("timing", timing);
}

// The existing event queue and dispatcher state.
static const char* control_api(const char* request) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (api != "control.processEvents" && api != "audio.isProcessing" &&
            api != "scope.target" && api != "scope.setTarget" &&
            api != "test.observe" && api != "test.notes" &&
            api != "test.loopTrace" &&
            api != "test.pluginState" && api != "test.instances" && api != "track.volume" &&
            api != "track.setVolume" && api != "track.setDormant" && api != "midi.controlChange") return nullptr;
        if (!session) throw std::runtime_error("No session");
        auto value = choc::value::Value();
        if (api == "midi.controlChange") {
            const auto instance = args["instanceId"].get<int32_t>();
            const auto controller = args["controller"].get<int32_t>();
            const auto cc = args["value"].get<int32_t>();
            if (controller < 0 || controller > 127 || cc < 0 || cc > 127)
                throw std::runtime_error("Expected MIDI controller and value in 0..127");
            const auto ump = umppi::UmpFactory::midi2CC(0, 0, controller,
                static_cast<uint32_t>(static_cast<uint64_t>(cc) * UINT32_MAX / 127));
            uapmd_ump_t words[] = {static_cast<uapmd_ump_t>(ump >> 32), static_cast<uapmd_ump_t>(ump)};
            uapmd_app::AppModel::instance().sequencer().engine()->enqueueUmp(instance, words, sizeof(words), 0);
        } else if (api == "test.observe") {
            value = choc::value::createObject("");
            observe_into(value);
        } else if (api == "test.notes") {
            value = held_notes();
        } else if (api == "test.loopTrace") {
            value = choc::value::createObject("");
            loop_trace_into(value, args["start"].getWithDefault<bool>(false));
        } else if (api == "test.instances") {
            value = choc::value::createEmptyArray();
            for (auto id : uapmd_app::AppModel::instance().sequencer().engine()->pluginHost()->instanceIds())
                value.addArrayElement(id);
        } else if (api == "test.pluginState") {
            value = choc::value::createString(session->readState(args["instanceId"].get<int32_t>()));
        } else if (api == "track.volume" || api == "track.setVolume" || api == "track.setDormant") {
            // The same track gain as uapmd's track volume slider.
            auto& engine = *uapmd_app::AppModel::instance().sequencer().engine();
            const auto trackIndex = args["trackIndex"].get<int32_t>();
            if (trackIndex < 0 || static_cast<size_t>(trackIndex) >= engine.tracks().size())
                throw std::runtime_error("Invalid track index");
            if (api == "track.setDormant") {
                const auto dormant = args["dormant"].get<bool>();
                if (engine.tracks()[trackIndex]->trackGain() != 0)
                    throw std::runtime_error("Dormancy changes require a silent track");
                // CLAP stopProcessing resets held voices/tails. Exclude callbacks
                // while clearing queued host input/output and changing bypass.
                engine.resetTrackProcessingState(trackIndex, dormant, [&] {
                    for (auto& sequence : session->sequences)
                        if (sequence) sequence->synthReset(trackIndex);
                    for (const auto& [id, node] : engine.tracks()[trackIndex]->graph().plugins()) {
                        node->clearQueuedEvents();
                        node->requestStopFlush();
                        node->bypassed(dormant);
                    }
                });
            }
            if (api == "track.setVolume") {
                if (!engine.commands().setTrackGain(trackIndex, args["volume"].get<double>()))
                    throw std::runtime_error("Track volume was not changed");
                engine.markTrackDirty(trackIndex);
            }
            value = choc::value::createFloat64(engine.tracks()[static_cast<size_t>(trackIndex)]->trackGain());
        } else if (api == "control.processEvents") {
            remidy::EventLoop::processQueuedTasks();
            repl_finish_pending_freeze();
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        } else if (api == "scope.target") {
            value = choc::value::createInt32(repl_notes::target.load(std::memory_order_relaxed));
        } else if (api == "scope.setTarget") {
            repl_notes::target.store(args["instanceId"].get<int32_t>(), std::memory_order_relaxed);
        } else {
            value = choc::value::createBool(uapmd_app::AppModel::instance().sequencer().isAudioPlaying() != 0);
        }
        result = choc::json::toString(choc::value::createObject("", "ok", true, "value", value));
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}

// Main/control thread only. Reply is owned by the DLL until the next call;
// the Rust caller copies it immediately. No exceptions cross the C ABI.
extern "C" __declspec(dllexport) const char* repl_request(const char* request) noexcept {
    try {
        auto args = choc::json::parse(request);
        auto command = args["command"].get<std::string>();
        auto result = choc::value::createObject("");
        result.setMember("ok", true);
        if (command == "open") {
            if (session) throw std::runtime_error(repl_localized("native_02"));
            auto diagnostics = std::filesystem::u8path(args["diagnostics"].get<std::string>());
            FILE* stream{};
            if (_wfreopen_s(&stream, (diagnostics.wstring() + L".stdout.log").c_str(), L"w", stdout) != 0 ||
                _wfreopen_s(&stream, (diagnostics.wstring() + L".stderr.log").c_str(), L"w", stderr) != 0)
                throw std::runtime_error(repl_localized("native_03"));
            session = std::make_unique<Session>(args["data"].get<std::string>());
            session->initialize();
            result.setMember("bootstrap", session->runtime->evaluateScript("typeof uapmd.audio.setEngineEnabled"));
            result.setMember("uapmdRevision", std::string(REPL_UAPMD_REVISION));
            result.setMember("catalogFromCache", session->catalog_from_cache);
            auto* device = uapmd::defaultDeviceIODispatcher()->audio();
            pad_gain_rate.store(device ? device->sampleRate() : 0.0, std::memory_order_relaxed);
            result.setMember("deviceOpened", device != nullptr);
            if (device) {
                result.setMember("sampleRate", device->sampleRate());
                result.setMember("blockSize", static_cast<int64_t>(device->preferredCallbackSize()));
                result.setMember("outputChannels", static_cast<int64_t>(device->outputChannels()));
            }
        } else if (command == "close") {
            session.reset();
            std::fflush(nullptr);
        } else {
            if (!session || !session->runtime) throw std::runtime_error(repl_localized("native_04"));
            remidy::EventLoop::processQueuedTasks();
            session->windows.pump();
            repl_finish_pending_freeze();
            if (command == "cli-pump") {
                // CLI has no winit event loop. Drain this control thread's
                // windows (including plugin-owned hidden callback windows).
                // Normal GUI callers never send this command.
                MSG message{};
                bool quit = false;
                for (int count = 0; count < 64 && PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE); ++count) {
                    if (message.message == WM_QUIT) { quit = true; break; }
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                result.setMember("quit", quit);
            } else if (command == "poll") {
                result.setMember("scanned", session->scanning_done);
                result.setMember("scanError", session->scan_error);
                result.setMember("catalogCount", session->runtime->evaluateScript("uapmd.catalog.getCount()"));
                result.setMember("stateReady", session->state_ready);
                result.setMember("preparationError", session->preparation_error);
            } else if (command == "scan") {
                auto& model = uapmd_app::AppModel::instance();
                model.stopPluginScanning();
                session->scanning_done = false;
                session->scan_error.clear();
                model.performPluginScanning(false, uapmd_app::AppModel::PluginScanRequest::InProcess, 0.0, false);
            } else if (command == "refresh") {
                session->startInitialScan();
            } else if (command == "request-state") {
                session->requestState(args["instanceId"].get<int32_t>());
            } else if (command == "state") {
                if (!session->state_ready) throw std::runtime_error(repl_localized("native_06"));
                result.setMember("hex", session->stateHex());
            } else if (command == "notes") {
                result.setMember("notes", held_notes());
            } else if (command == "observe") {
                observe_into(result);
            } else if (command == "parameter-safety") {
                result = parameter_safety(args["instanceId"].get<int32_t>(), args["parameterId"].get<int32_t>());
                result.setMember("ok", true);
            } else if (command == "eval" || command == "timeline-view") {
                session->windows.takeError();
                auto value = session->runtime->evaluateScript(args["code"].get<std::string>());
                auto uiError = session->windows.takeError();
                if (!uiError.empty()) throw std::runtime_error(repl_format1("native_16", "error", uiError));
                result.setMember("result", value);
                if (command == "timeline-view") result.setMember("view", session->timeline_view.read(
                    *uapmd_app::AppModel::instance().sequencer().engine()));
            } else throw std::runtime_error(repl_format1("native_17", "command", command));
        }
        reply = choc::json::toString(result, false);
    } catch (const std::exception& error) {
        auto result = choc::value::createObject("");
        result.setMember("ok", false);
        result.setMember("error", std::string(error.what()));
        reply = choc::json::toString(result, false);
    } catch (...) {
        reply = choc::json::toString(choc::value::createObject("", "ok", false, "error", std::string(repl_localized("native_09"))));
    }
    return reply.c_str();
}
