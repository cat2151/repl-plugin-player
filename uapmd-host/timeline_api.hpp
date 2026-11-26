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
        // TimelineState's own loop fields compile but no longer drive playback.
        engine.timeline().loopRegion({enabled, start, end});
        result = "{\"ok\":true,\"value\":null}";
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}
