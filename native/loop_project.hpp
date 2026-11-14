#pragma once
#include "loop_playback.hpp"
#include <uapmd-data/detail/project/ProjectSerializationExtension.hpp>

namespace repl_loop {
// Loop plans are not timeline clips. Keep them in UAPMD's archive extension,
// together with the project that owns their track identities and instruments.
class Project final : public uapmd::ProjectSerializationExtension {
    Playback& playback;
    uapmd::SequencerEngine* engine{};
    std::unique_ptr<Plan> loaded;
    double loadedBpm{120};
public:
    explicit Project(Playback& value) : playback(value) {}
    void attach(uapmd::SequencerEngine& value) {
        engine = &value;
        engine->timeline().addProjectSerializationExtension(*this);
    }
    void detach() { engine->timeline().removeProjectSerializationExtension(*this); }
    std::string_view extensionId() const override { return "repl-loop"; }
    bool saveProjectExtensionData(uapmd::ProjectSerializationWriteContext& context, std::string& error) override {
        // Capture the latest submitted edit, including a pending boundary update.
        auto data = playback.view(false);
        data.setMember("version", 1);
        data.setMember("sampleRate", engine->currentSampleRate());
        const auto json = choc::json::toString(data);
        return context.writeExtensionFile(extensionId(), "loop.json", {json.begin(), json.end()}, error);
    }
    bool loadProjectExtensionData(uapmd::ProjectSerializationReadContext& context, std::string& error) override {
        loaded.reset();
        try {
            const auto path = context.projectDirectory() / "extensions" / "repl-loop" / "loop.json";
            if (!std::filesystem::exists(path)) return true;
            const auto bytes = context.readExtensionFile(extensionId(), "loop.json", error);
            if (!bytes) return false;
            const auto data = choc::json::parse(std::string(bytes->begin(), bytes->end()));
            if (data["version"].get<int>() != 1) throw std::runtime_error("Unsupported loop project version");
            const auto rate = data["sampleRate"].get<double>();
            loadedBpm = data["bpm"].get<double>();
            if (!std::isfinite(rate) || rate <= 0 || !std::isfinite(loadedBpm) || loadedBpm <= 0)
                throw std::runtime_error("Invalid saved loop timing");
            if (!data.hasObjectMember("lengthSamples")) return true;
            const auto length = data["lengthSamples"].get<int64_t>();
            if (length == 0) return true;
            if (length < 0 || length > 1000000000000LL) throw std::runtime_error("Invalid saved loop length");
            const double scale = engine->currentSampleRate() / rate;
            auto plan = std::make_unique<Plan>();
            plan->length = Playback::sampleLength(length / rate * loadedBpm / 60.0, loadedBpm, engine->currentSampleRate());
            const auto parts = data["parts"];
            if (!parts.isArray() || parts.size() > 32) throw std::runtime_error("Invalid saved loop parts");
            const auto tracks = engine->timeline().tracks();
            for (const auto item : parts) {
                Part part;
                part.track = item["trackId"].get<std::string>();
                if (std::none_of(tracks.begin(), tracks.end(), [&](auto* t) { return t->referenceId() == part.track; }) ||
                    std::any_of(plan->parts.begin(), plan->parts.end(), [&](const auto& p) { return p.track == part.track; }))
                    throw std::runtime_error("Invalid saved loop track");
                const auto events = item["events"];
                if (!events.isArray() || events.size() > 16384) throw std::runtime_error("Invalid saved loop events");
                int64_t previous = -1;
                for (const auto value : events) {
                    const auto sample = value["sample"].get<int64_t>();
                    const auto words = value["words"];
                    if (sample < previous || sample < 0 || sample >= length || !words.isArray() || words.size() < 1 || words.size() > 2)
                        throw std::runtime_error("Invalid saved loop event");
                    Event event;
                    event.sample = std::min(plan->length - 1, static_cast<int64_t>(std::llround(sample * scale)));
                    event.size = words.size();
                    for (uint32_t i = 0; i < event.size; ++i) {
                        const auto word = words[i].get<int64_t>();
                        if (word < 0 || word > UINT32_MAX) throw std::runtime_error("Invalid saved loop word");
                        event.words[i] = static_cast<uint32_t>(word);
                    }
                    const auto type = event.words[0] >> 28;
                    const auto status = (event.words[0] >> 20) & 15;
                    if (!((type == 2 && event.size == 1) || (type == 4 && event.size == 2)) ||
                        ((event.words[0] >> 24) & 15) || (status != 8 && status != 9))
                        throw std::runtime_error("Invalid saved loop note");
                    part.events.push_back(event);
                    previous = sample;
                }
                plan->parts.push_back(std::move(part));
            }
            loaded = std::move(plan);
            return true;
        } catch (const std::exception& exception) {
            error = std::string("repl-loop: ") + exception.what();
            return false;
        }
    }
    // Called after upstream restoration, which also invokes playbackStopped().
    void projectReplaced() {
        playback.projectReplaced(*engine);
        if (!loaded) return;
        playback.bpm = loadedBpm;
        playback.sampleRate = engine->currentSampleRate();
        playback.period.store(loaded->length);
        loaded->revision = ++playback.revision;
        playback.stoppedRevision = loaded->revision;
        playback.plans.publish(std::move(loaded));
    }
};
}
