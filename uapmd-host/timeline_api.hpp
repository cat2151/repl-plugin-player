#pragma once

// Timeline operations exposed by UAPMD in C++ but missing from its JS API.
static const char* timeline_api(const char* request, bool sessionOpen) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (api != "timeline.setLoop" && api != "timeline.clearLoop" && api != "timeline.addAudioClip") return nullptr;
        if (!sessionOpen) throw std::runtime_error("No session");
        auto& model = uapmd_app::AppModel::instance();
        auto& engine = *model.sequencer().engine();
        if (api == "timeline.addAudioClip") {
            const auto trackId = args["trackId"].get<std::string>();
            const auto track = engine.document().view().getTrack(trackId);
            const auto filepath = args["filepath"].get<std::string>();
            uapmd::TimelinePosition position{};
            position.samples = args["positionSamples"].get<int64_t>();
            if (!track || track->trackIndex < 0 || position.samples < 0 || filepath.empty())
                throw std::runtime_error("Invalid audio clip arguments");
            auto reader = uapmd::createAudioFileReaderFromPath(filepath);
            if (!reader) throw std::runtime_error("Could not open WAV: " + filepath);
            auto added = model.addClipToTrack(track->trackIndex, position, std::move(reader), filepath);
            if (!added.success) throw std::runtime_error(added.error);
            result = R"({"ok":true,"value":null})";
            return result.c_str();
        }
        const bool enabled = api == "timeline.setLoop";
        const auto start = enabled ? args["startSamples"].get<int64_t>() : 0;
        const auto end = enabled ? args["endSamples"].get<int64_t>() : 0;
        if (enabled && (start < 0 || end <= start)) throw std::runtime_error("Invalid loop range");
        // TimelineState's own loop fields compile but no longer drive playback.
        engine.timeline().loopRegion({enabled, start, end});
        result = "{\"ok\":true,\"value\":null}";
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}
