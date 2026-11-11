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
#include "localization.hpp"
#include "plugin_windows.hpp"
#include "parameter_safety.hpp"
#include "loop_playback.hpp"
#include "timeline_view.hpp"
#include "scope_capture.hpp"
#include "../patches/uapmd/loop-playback/native_regression.hpp"

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

// Native exceptions must not unwind through QuickJS callback frames.
static thread_local std::string preset_error;
extern "C" void repl_preset_error(const char* error) { preset_error = error; }

using ScriptApi = const char* (*)(const char*);
static ScriptApi script_api{};
static const char* loop_api(const char* request);
static const char* control_api(const char* request);
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
    if (auto result = loop_api(request)) return result;
    if (auto result = control_api(request)) return result;
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
    repl_loop::Playback loop_playback;
    TimelineView timeline_view;
    ControlLoop loop;
    remidy::EventLoop* previous{};
    bool model_created{};
    bool scanning_done{};
    std::string scan_error;
    int32_t instance_id{-1};
    bool creating{};
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
        repl_loop::instance = &loop_playback;
        model.sequencer().engine()->addPlaybackEngineExtension(loop_playback);
        windows.attach();
        model.scanningCompleted.push_back([this](bool ok, std::string error) {
            remidy::EventLoop::enqueueTaskOnMainThread([this, ok, error] {
                scanning_done = true;
                scan_error = ok ? "" : error;
            });
        });
        model.notifyUiReady();
        model.notifyPersistentStorageReady();
    }
    void requestState(int32_t target = -1) {
        if (state_pending) throw std::runtime_error(repl_localized("native_00"));
        auto* instance = uapmd_app::AppModel::instance().sequencer().engine()->getPluginInstance(target < 0 ? instance_id : target);
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
    ~Session() {
        if (model_created) {
            auto& model = uapmd_app::AppModel::instance();
            model.stopPluginScanning();
            while (creating || state_pending || model.historyState().busy) {
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
            model.sequencer().engine()->removePlaybackEngineExtension(loop_playback);
            repl_loop::instance = nullptr;
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

// Only the existing event queue and dispatcher state; waiting policy stays in JS.
static const char* control_api(const char* request) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (api != "control.processEvents" && api != "audio.isProcessing") return nullptr;
        if (!session) throw std::runtime_error("No session");
        auto value = choc::value::Value();
        if (api == "control.processEvents") {
            remidy::EventLoop::processQueuedTasks();
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        } else {
            value = choc::value::createBool(uapmd_app::AppModel::instance().sequencer().isAudioPlaying() != 0);
        }
        result = choc::json::toString(choc::value::createObject("", "ok", true, "value", value));
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}

static const char* loop_api(const char* request) {
    static thread_local std::string result;
    try {
        auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (!api.starts_with("timeline.loop.")) return nullptr;
        if (!session) throw std::runtime_error("No session");
        auto& playback = session->loop_playback;
        auto& engine = *uapmd_app::AppModel::instance().sequencer().engine();
        auto value = choc::value::createObject("");
        if (api == "timeline.loop.configure") playback.configure(args["beats"].get<double>(), args["bpm"].get<double>(), engine);
        else if (api == "timeline.loop.publish") value = choc::value::createInt64(static_cast<int64_t>(playback.publish(args["parts"], engine,
            args["beats"].getWithDefault<double>(0), args["bpm"].getWithDefault<double>(0))));
        else if (api == "timeline.loop.status") value = playback.status();
        else if (api == "timeline.loop.clear") playback.clear(engine);
        else throw std::runtime_error("Unknown loop API");
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
            if (command == "loop-checks") {
                repl_loop::Regression::run();
            } else if (command == "cli-pump") {
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
            } else if (command == "loop-trace") {
                if (args["start"].getWithDefault<bool>(false)) {
                    if (uapmd_app::AppModel::instance().sequencer().isAudioPlaying())
                        throw std::runtime_error("Stop audio before starting input trace");
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
            } else if (command == "poll") {
                result.setMember("scanned", session->scanning_done);
                result.setMember("scanError", session->scan_error);
                result.setMember("catalogCount", session->runtime->evaluateScript("uapmd.catalog.getCount()"));
                result.setMember("instanceId", session->instance_id);
                result.setMember("creating", session->creating);
                result.setMember("stateReady", session->state_ready);
                result.setMember("preparationError", session->preparation_error);
            } else if (command == "scan") {
                auto& model = uapmd_app::AppModel::instance();
                model.stopPluginScanning();
                session->scanning_done = false;
                session->scan_error.clear();
                model.performPluginScanning(false, uapmd_app::AppModel::PluginScanRequest::InProcess, 0.0, false);
            } else if (command == "create") {
                if (session->creating || session->instance_id >= 0)
                    throw std::runtime_error(repl_localized("native_05"));
                session->creating = true;
                uapmd_app::AppModel::instance().createPluginInstanceAsync("CLAP", args["pluginId"].get<std::string>(), 0, {},
                    [owner = session.get()](const uapmd_app::AppModel::PluginInstanceResult& completed) {
                        auto id = completed.instanceId;
                        auto error = completed.error;
                        remidy::EventLoop::enqueueTaskOnMainThread([owner, id, error] {
                            owner->creating = false;
                            owner->instance_id = id;
                            repl_notes::target.store(id);
                            repl_notes::publish(0, 0);
                            owner->preparation_error = error;
                            if (id < 0 || !error.empty()) return;
                            owner->runtime->evaluateScript("globalThis.surgeId = " + std::to_string(id));
                            owner->runtime->registerAllParameterListeners();
                            owner->runtime->registerAllMetadataListeners();
                            owner->requestState();
                        });
                    });
            } else if (command == "request-state") {
                session->requestState(args["instanceId"].getWithDefault<int32_t>(session->instance_id));
            } else if (command == "state") {
                if (!session->state_ready) throw std::runtime_error(repl_localized("native_06"));
                std::string hex;
                constexpr char digits[] = "0123456789abcdef";
                for (auto byte : session->state) { hex += digits[byte >> 4]; hex += digits[byte & 15]; }
                result.setMember("hex", hex);
            } else if (command == "notes") {
                auto& model = uapmd_app::AppModel::instance();
                const auto id = session->instance_id;
                auto* instance = id >= 0 ? model.sequencer().engine()->getPluginInstance(id) : nullptr;
                auto notes = choc::value::createEmptyArray();
                std::array<uint64_t, 2> bits{};
                if (!repl_notes::read(bits)) throw std::runtime_error(repl_localized("native_07"));
                // A stopped device has no live input. Also avoid retaining a
                // destroyed instrument's last snapshot.
                if (instance && model.sequencer().isAudioPlaying()) {
                    for (int pitch = 0; pitch < 128; ++pitch)
                        if (bits[pitch / 64] & (uint64_t{1} << (pitch % 64)))
                            notes.addArrayElement(pitch);
                }
                result.setMember("available", instance != nullptr);
                result.setMember("instanceId", id);
                result.setMember("notes", notes);
            } else if (command == "observe") {
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
            } else if (command == "parameter-safety") {
                result = parameter_safety(args["instanceId"].get<int32_t>(), args["parameterId"].get<int32_t>());
                result.setMember("ok", true);
            } else if (command == "eval" || command == "timeline-view") {
                session->windows.takeError();
                preset_error.clear();
                auto value = session->runtime->evaluateScript(args["code"].get<std::string>());
                if (!preset_error.empty()) throw std::runtime_error(repl_format1("native_15", "error", preset_error));
                auto uiError = session->windows.takeError();
                if (!uiError.empty()) throw std::runtime_error(repl_format1("native_16", "error", uiError));
                result.setMember("result", value);
                if (command == "timeline-view") result.setMember("view", session->timeline_view.read(
                    *uapmd_app::AppModel::instance().sequencer().engine(), session->loop_playback));
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
