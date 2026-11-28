// Starts playback with a latency track, so the engine prerolls, and counts the
// note-ons of a song-start note that reach each track input.
#include <uapmd-engine/uapmd-engine.hpp>
#include <iostream>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>

// The app's UAPMD host DLL owns these observers.
void repl_capture_scope(int, remidy::AudioProcessContext&) noexcept {}

namespace {
using namespace uapmd_plugin_hosting;

constexpr int32_t kSampleRate = 48000;
constexpr size_t kEngineFrames = 1024;  // the preroll quantum
constexpr int32_t kBlockFrames = 256;
constexpr uint32_t kLatency = 32;
constexpr int kTracks = 2;

class Buses final : public remidy::PluginAudioBuses {
public:
    Buses() { inputs_.push_back(&input_); outputs_.push_back(&output_); }
    bool hasEventInputs() override { return true; }
    bool hasEventOutputs() override { return false; }
    const std::vector<remidy::AudioBusConfiguration*>& audioInputBuses() const override { return inputs_; }
    const std::vector<remidy::AudioBusConfiguration*>& audioOutputBuses() const override { return outputs_; }
private:
    remidy::AudioBusDefinition in_def_{"Input", remidy::AudioBusRole::Main, {remidy::AudioChannelLayout::stereo()}};
    remidy::AudioBusDefinition out_def_{"Output", remidy::AudioBusRole::Main, {remidy::AudioChannelLayout::stereo()}};
    remidy::AudioBusConfiguration input_{in_def_}, output_{out_def_};
    std::vector<remidy::AudioBusConfiguration*> inputs_, outputs_;
};

class Parameters final : public remidy::PluginParameterSupport {
public:
    std::vector<remidy::PluginParameter*>& parameters() override { return none_; }
    std::vector<remidy::PluginParameter*>& perNoteControllers(
        remidy::PerNoteControllerContextTypes, remidy::PerNoteControllerContext) override { return none_; }
    remidy::StatusCode setParameter(uint32_t, double) override { return remidy::StatusCode::OK; }
    remidy::StatusCode enqueueParameterRT(uint32_t, double, uint64_t) override { return remidy::StatusCode::OK; }
    remidy::StatusCode getParameter(uint32_t, double* value) override { *value = 0; return remidy::StatusCode::OK; }
    remidy::StatusCode setPerNoteController(remidy::PerNoteControllerContext, uint32_t, double) override {
        return remidy::StatusCode::OK;
    }
    remidy::StatusCode enqueuePerNoteControllerRT(remidy::PerNoteControllerContext, uint32_t, double, uint64_t) override {
        return remidy::StatusCode::OK;
    }
    remidy::StatusCode getPerNoteController(remidy::PerNoteControllerContext, uint32_t, double* value) override {
        *value = 0;
        return remidy::StatusCode::OK;
    }
    std::string valueToString(uint32_t, double) override { return {}; }
    std::string valueToStringPerNote(remidy::PerNoteControllerContext, uint32_t, double) override { return {}; }
private:
    std::vector<remidy::PluginParameter*> none_;
};

// A pass-through plugin whose only property is its reported latency.
class LatencyPlugin final : public AudioPluginInstanceAPI {
public:
    std::string& displayName() const override { return name_; }
    std::string& formatName() const override { return format_; }
    std::string& pluginId() const override { return id_; }
    bool bypassed() const override { return false; }
    void bypassed(bool) override {}
    uapmd_status_t startProcessing() override { return 0; }
    uapmd_status_t stopProcessing() override { return 0; }
    uapmd_status_t processAudio(remidy::AudioProcessContext& process) override {
        process.copyInputsToOutputs();
        return 0;
    }
    uint32_t latencyInSamples() const override { return kLatency; }
    double tailLengthInSeconds() const override { return 0; }
    bool requiresReplacingProcess() const override { return false; }
    std::vector<ParameterMetadata> parameterMetadataList() override { return {}; }
    std::vector<ParameterMetadata> perNoteControllerMetadataList(remidy::PerNoteControllerContextTypes, uint32_t) override {
        return {};
    }
    std::vector<PresetsMetadata> presetMetadataList() override { return {}; }
    void loadPreset(int32_t) override {}
    void loadPreset(int32_t, std::function<void(std::string, void*)> completed) override { completed({}, nullptr); }
    std::vector<uint8_t> saveStateSync() override { return {}; }
    void loadStateSync(std::vector<uint8_t>&) override {}
    void requestState(StateContextType, bool, void* context,
                      std::function<void(std::vector<uint8_t>, std::string, void*)> receiver) override {
        receiver({}, {}, context);
    }
    void loadState(std::vector<uint8_t>, StateContextType, bool, void* context,
                   std::function<void(std::string, void*)> completed) override {
        completed({}, context);
    }
    double getParameterValue(int32_t) override { return 0; }
    void setParameterValue(int32_t, double) override {}
    void enqueueParameterValueRT(int32_t, double, uapmd_timestamp_t) override {}
    std::string getParameterValueString(int32_t, double) override { return {}; }
    void setPerNoteControllerValue(uint8_t, uint8_t, double) override {}
    void enqueuePerNoteControllerValueRT(uint8_t, uint8_t, double, uapmd_timestamp_t) override {}
    std::string getPerNoteControllerValueString(uint8_t, uint8_t, double) override { return {}; }
    bool hasUISupport() override { return false; }
    bool createUI(bool, void*, std::function<bool(uint32_t, uint32_t)>) override { return false; }
    void destroyUI() override {}
    bool showUI() override { return false; }
    void hideUI() override {}
    bool isUIVisible() const override { return false; }
    bool setUISize(uint32_t, uint32_t) override { return false; }
    bool getUISize(uint32_t&, uint32_t&) override { return false; }
    bool canUIResize() override { return false; }
    remidy::PluginParameterSupport* parameterSupport() override { return &parameters_; }
    remidy::PluginAudioBuses* audioBuses() override { return &buses_; }
    remidy::EventListenerId addTimingInfoChangeListener(std::function<void(remidy::PluginTimingInfoChange)>) override {
        return 0;
    }
    void removeTimingInfoChangeListener(remidy::EventListenerId) override {}
private:
    mutable std::string name_{"Latency"}, format_{"Test"}, id_{"latency"};
    Parameters parameters_;
    Buses buses_;
};

class Host final : public AudioPluginHostingAPI {
public:
    std::vector<AudioPluginCatalogEntry> pluginCatalogEntries() override { return {}; }
    void savePluginCatalogToFile(std::filesystem::path) override {}
    void performPluginScanning(bool) override {}
    void reloadPluginCatalogFromCache() override {}
    void createPluginInstance(uint32_t, uint32_t, std::optional<uint32_t>, std::optional<uint32_t>, bool,
                              std::string&, std::string&,
                              std::function<void(int32_t, std::string)>&& callback) override {
        const int32_t id = next_++;
        instances_[id] = std::make_unique<LatencyPlugin>();
        callback(id, {});
    }
    void deletePluginInstance(int32_t id) override { instances_.erase(id); }
    AudioPluginInstanceAPI* getInstance(int32_t id) override {
        const auto it = instances_.find(id);
        return it == instances_.end() ? nullptr : it->second.get();
    }
    void addPluginFormat(AudioPluginFormat*) override {}
    std::vector<AudioPluginFormat*> pluginFormats() override { return {}; }
    remidy::EventListenerId addPluginStateChangeListener(std::function<void(int32_t)>) override { return 1; }
    void removePluginStateChangeListener(remidy::EventListenerId) override {}
    std::vector<int32_t> instanceIds() override {
        std::vector<int32_t> ids;
        for (const auto& entry : instances_) ids.push_back(entry.first);
        return ids;
    }
private:
    int32_t next_{100};
    std::map<int32_t, std::unique_ptr<LatencyPlugin>> instances_;
};

struct Recorder final : uapmd::AudioProcessingEventHandler {
    int ons[kTracks]{};
    bool supportsParallelTrackProcessing() const noexcept override { return true; }
    void beforeTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        if (event.track_index < 0 || event.track_index >= kTracks) return;
        auto& input = event.context.eventIn();
        const auto* data = static_cast<const uint32_t*>(input.getMessages());
        const auto words = input.position() / sizeof(uint32_t);
        for (size_t i = 0; i < words; i += (data[i] >> 28) == 4 ? 2 : 1)
            if ((data[i] >> 28) == 4 && ((data[i] >> 20) & 15) == 9) ++ons[event.track_index];
    }
};

void addSongStartNote(uapmd::SequencerEngine& engine, int track) {
    uapmd::ClipData clip;
    clip.clipType = uapmd::ClipType::Midi;
    clip.durationSamples = 480000;
    clip.sourceNodeInstanceId = 7 + track;
    clip.setTimeReference(uapmd::TimeReference::fromContainerStart({}, 0), kSampleRate);
    // Note on at 0, note off after 0.2 s: one note-on in the whole run.
    auto node = std::make_unique<uapmd::MidiClipSourceNode>(7 + track,
        std::vector<uapmd_ump_t>{0x40903c00u, 0x80000000u, 0x40803c00u, 0u},
        std::vector<uint64_t>{0, 0, 9600, 9600}, 480, 120.0, static_cast<double>(kSampleRate),
        std::vector<uapmd::MidiTempoChange>{}, std::vector<uapmd::MidiTimeSignatureChange>{});
    if (engine.timeline().tracks()[track]->addClip(clip, std::move(node)) < 0)
        throw std::runtime_error("clip not added");
}
}

int main() {
    try {
        auto engine = uapmd::SequencerEngine::createWithPluginHost(
            kSampleRate, kEngineFrames, 65536, std::make_unique<Host>());
        engine->setEngineActive(true);
        for (int track = 0; track < kTracks; ++track)
            if (engine->addEmptyTrack() != track) throw std::runtime_error("track not added");
        std::string format = "Test", id = "latency";
        int32_t instance = -1;
        engine->addPluginToTrack(1, format, id, [&](int32_t created, int32_t, std::string error) {
            if (error.empty()) instance = created;
        });
        if (instance < 0) throw std::runtime_error("plugin not added");
        for (int track = 0; track < kTracks; ++track) addSongStartNote(*engine, track);
        const auto lead = engine->trackRenderLeadInSamples(1);
        if (lead == 0) throw std::runtime_error("latency track has no render lead; no preroll to test");

        Recorder recorder;
        engine->addAudioProcessingEventHandler(recorder);
        engine->startPlayback();
        const auto preroll = engine->playbackPosition() - engine->renderPlaybackPosition();
        remidy::AudioProcessContext process(engine->data().masterContext(), 65536);
        process.configureMainBus(2, 2, kBlockFrames);
        process.frameCount(kBlockFrames);
        // Ends before the note-off, so the song-start note-on is the only one.
        for (int block = 0; block < 32; ++block)
            if (engine->processAudio(process) != 0) throw std::runtime_error("processAudio failed");
        engine->stopPlayback();
        engine->removeAudioProcessingEventHandler(recorder);

        bool ok = true;
        for (int track = 0; track < kTracks; ++track) {
            const bool once = recorder.ons[track] == 1;
            ok &= once;
            std::cout << (once ? "PASS " : "FAIL ") << "track " << track << " (render lead "
                      << engine->trackRenderLeadInSamples(track) << ", preroll " << preroll
                      << "): song-start note-ons " << recorder.ons[track] << '\n';
        }
        return ok ? 0 : 1;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
