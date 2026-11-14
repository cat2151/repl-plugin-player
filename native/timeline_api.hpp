#pragma once

// UAPMD exposes loop state in C++, but has no JS setter. Content remains clips.
static const char* timeline_api(const char* request, bool sessionOpen) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (api != "timeline.setLoop" && api != "timeline.clearLoop") return nullptr;
        if (!sessionOpen) throw std::runtime_error("No session");
        const bool enabled = api == "timeline.setLoop";
        const auto start = enabled ? args["startSamples"].get<int64_t>() : 0;
        const auto end = enabled ? args["endSamples"].get<int64_t>() : 0;
        if (enabled && (start < 0 || end <= start)) throw std::runtime_error("Invalid loop range");
        auto& engine = *uapmd_app::AppModel::instance().sequencer().engine();
        // This upstream operation excludes callbacks. An absent track skips buffer resets;
        // only its synchronous transition is needed for transport state.
        engine.resetTrackProcessingState(-1, false, [&] {
            auto& state = engine.timeline().state();
            state.loopEnabled = enabled;
            state.loopStart.samples = start;
            state.loopEnd.samples = end;
        });
        result = "{\"ok\":true,\"value\":null}";
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}
