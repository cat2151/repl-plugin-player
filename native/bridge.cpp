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
#include "localization.hpp"
#include "plugin_windows.hpp"
#include "parameter_safety.hpp"
#include "timeline_api.hpp"
#include "timeline_view.hpp"
#include "piano_roll_api.hpp"
#include "project_api.hpp"
#include "scope_capture.hpp"

using ScopeCallback = void (*)(void*, const void*, const void*, size_t, uint32_t, const uint32_t*, size_t);
static ScopeCallback scope_callback{};
static void* scope_context{};
extern "C" __declspec(dllexport) void repl_set_scope(ScopeCallback callback, void* context) {
    scope_callback = callback;
    scope_context = context;
}
void repl_capture_scope(int32_t instance, uapmd::AudioProcessContext& process) noexcept {
    if (!scope_callback || instance != repl_notes::target.load(std::memory_order_relaxed) ||
        process.frameCount() <= 0 || process.audioOutBusCount() == 0 || process.outputChannelCount(0) == 0) return;
    const bool doubles = process.masterContext().audioDataType() == remidy::AudioContentType::Float64;
    const auto channels = process.outputChannelCount(0);
    const void* left = doubles ? static_cast<const void*>(process.getDoubleOutBuffer(0, 0)) : process.getFloatOutBuffer(0, 0);
    const void* right = channels > 1
        ? (doubles ? static_cast<const void*>(process.getDoubleOutBuffer(0, 1)) : process.getFloatOutBuffer(0, 1)) : left;
    if (!left || !right) return;
    scope_callback(scope_context, left, right, static_cast<size_t>(process.frameCount()), doubles ? 1u : 0u,
        static_cast<const uint32_t*>(process.eventIn().getMessages()), process.eventIn().position() / sizeof(uint32_t));
}

using ScriptApi = const char* (*)(const char*);
static ScriptApi script_api{};
static const char* control_api(const char* request);
static bool session_open();
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
extern "C" __declspec(dllexport) void repl_set_script_api(ScriptApi callback) {
    script_api = callback;
}
// Called synchronously on the control thread; copy the Rust-owned reply immediately.
extern "C" const char* repl_script_api(const char* request) {
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
        model.scanningCompleted.push_back([this](bool ok, std::string error) {
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

// The master output's latest 256 samples (before soft clip / device mute) and the engine counters.
static void observe_into(choc::value::Value& result) {
    auto& model = uapmd_app::AppModel::instance();
    auto* engine = model.sequencer().engine();
    engine->audioPerformanceCounter().setEnabled(true);
    const auto counters = engine->audioPerformanceCounter().counters();
    uapmd::AudioProcessingTiming timing;
    while (engine->audioPerformanceCounter().tryDequeue(timing)) {}
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
            api != "test.pluginState" && api != "track.volume" &&
            api != "track.setVolume") return nullptr;
        if (!session) throw std::runtime_error("No session");
        auto value = choc::value::Value();
        if (api == "test.observe") {
            value = choc::value::createObject("");
            observe_into(value);
        } else if (api == "test.notes") {
            value = held_notes();
        } else if (api == "test.loopTrace") {
            value = choc::value::createObject("");
            loop_trace_into(value, args["start"].getWithDefault<bool>(false));
        } else if (api == "test.pluginState") {
            value = choc::value::createString(session->readState(args["instanceId"].get<int32_t>()));
        } else if (api == "track.volume" || api == "track.setVolume") {
            // The same track gain as uapmd's track volume slider.
            auto& engine = *uapmd_app::AppModel::instance().sequencer().engine();
            const auto trackIndex = args["trackIndex"].get<int32_t>();
            if (trackIndex < 0 || static_cast<size_t>(trackIndex) >= engine.tracks().size())
                throw std::runtime_error("Invalid track index");
            if (api == "track.setVolume") {
                if (!engine.commands().setTrackGain(trackIndex, args["volume"].get<double>()))
                    throw std::runtime_error("Track volume was not changed");
                engine.markTrackDirty(trackIndex);
            }
            value = choc::value::createFloat64(engine.tracks()[static_cast<size_t>(trackIndex)]->trackGain());
        } else if (api == "control.processEvents") {
            remidy::EventLoop::processQueuedTasks();
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
