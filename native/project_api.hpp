#pragma once
#include <future>

// Desktop-only path binding to UAPMD's archive save (project.save writes a
// directory project even when given a .uapmdz filename), and resolved-path load.
static const char* project_api(const char* request, bool open) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (api != "project.saveArchive" && api != "project.load") return nullptr;
        if (!open) throw std::runtime_error("No session");
        const auto path = args["filepath"].get<std::string>();
        if (path.empty()) throw std::runtime_error("Project path is empty");
        auto promise = std::make_shared<std::promise<uapmd_app::AppModel::ProjectResult>>();
        auto future = promise->get_future();
        auto& model = uapmd_app::AppModel::instance();
        if (api == "project.load") {
            model.loadProjectFromResolvedPath(std::filesystem::u8path(path),
                [promise](uapmd_app::AppModel::ProjectResult loaded) {
                    promise->set_value(std::move(loaded));
                });
        } else {
            // Desktop document handles identify filesystem paths. No picker is
            // needed for this explicit script destination, which may not exist yet.
            model.saveProjectToDocument({path, {}, {}},
                [promise](uapmd::DocumentIOResult saved) {
                    promise->set_value({saved.success, std::move(saved.error)});
                });
        }
        // Plugin state capture/restore posts completions to this control thread.
        while (future.wait_for(std::chrono::milliseconds(0)) != std::future_status::ready) {
            remidy::EventLoop::processQueuedTasks();
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
        const auto completed = future.get();
        if (!completed.success) throw std::runtime_error(completed.error);
        result = choc::json::toString(choc::value::createObject("", "ok", true, "value", path));
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}
