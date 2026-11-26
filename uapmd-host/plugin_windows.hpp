#pragma once

#include <remidy-gui/remidy-gui.hpp>
#include <uapmd-app-model/uapmd-app-model.hpp>
#include <map>
#include <limits>
#include <stdexcept>
#include <utility>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

// UI/control-thread only. Follow cat-plugin-player's embedded editor lifecycle:
// keep editors on hide, honor plug-in resize requests, destroy editors before parents.
class PluginWindows {
    struct Editor {
        std::unique_ptr<remidy::gui::ContainerWindow> window;
        bool created{};
        bool shown{};
        std::string error;
    };
    std::map<int32_t, Editor> editors;
    std::string pendingError;

    static auto* instance(int32_t id) {
        return uapmd_app::AppModel::instance().sequencer().engine()->getPluginInstance(id);
    }

    bool resize(int32_t id, uint32_t width, uint32_t height) {
        auto it = editors.find(id);
        if (it == editors.end() || !it->second.window || width == 0 || height == 0 ||
            width > static_cast<uint32_t>(std::numeric_limits<int>::max()) ||
            height > static_cast<uint32_t>(std::numeric_limits<int>::max()))
            return false;
        it->second.window->resize(static_cast<int>(width), static_cast<int>(height));
        return true;
    }

    void show(int32_t id) {
        auto* plugin = instance(id);
        if (!plugin) throw std::runtime_error(repl_localized("uapmd_host_10"));
        if (!plugin->hasUISupport()) throw std::runtime_error(repl_localized("uapmd_host_11"));
        auto& editor = editors[id];
        if (!editor.window) {
            const auto title = plugin->displayName() + " (" + plugin->formatName() + ")";
            editor.window = remidy::gui::ContainerWindow::create(title.c_str(), 800, 600, [id] {
                uapmd_app::AppModel::instance().hidePluginUI(id);
            });
            if (!editor.window || !editor.window->getHandle()) {
                editors.erase(id);
                throw std::runtime_error(repl_localized("uapmd_host_12"));
            }
            // As in cat-plugin-player, size changes originate in the plug-in.
            editor.window->setResizable(false);
        }
        editor.error.clear();
        editor.shown = false;
        editor.window->show(true);
        try {
            uapmd_app::AppModel::instance().showPluginUI(id, !editor.created, false,
                editor.window->getHandle(), [this, id](uint32_t w, uint32_t h) {
                    return resize(id, w, h);
                });
            if (!editor.shown)
                throw std::runtime_error(editor.error.empty() ? repl_localized("uapmd_host_13") : editor.error);
            uint32_t width{}, height{};
            if (plugin->getUISize(width, height)) resize(id, width, height);
        } catch (...) {
            editor.window->show(false);
            throw;
        }
    }

public:
    void attach() {
        auto& model = uapmd_app::AppModel::instance();
        model.uiShowRequested.push_back([this](int32_t id) {
            // QuickJS native callbacks do not catch C++ exceptions. Let the host
            // report failures only after evaluateScript has returned safely.
            try {
                show(id);
            } catch (const std::exception& error) {
                pendingError = error.what();
            } catch (...) {
                pendingError = repl_localized("uapmd_host_14");
            }
        });
        model.uiShown.push_back([this](const uapmd_app::AppModel::UIStateResult& result) {
            auto it = editors.find(result.instanceId);
            if (it == editors.end()) return;
            auto& editor = it->second;
            // Keep creation state even if showing failed, so a retry does not create twice.
            editor.created = editor.created || result.wasCreated;
            editor.shown = result.success && result.visible;
            editor.error = result.error;
        });
        model.uiHidden.push_back([this](const uapmd_app::AppModel::UIStateResult& result) {
            auto it = editors.find(result.instanceId);
            if (result.success && it != editors.end()) {
                it->second.shown = false;
                it->second.window->show(false);
            }
        });
        // AppModel removes the plug-in/editor first; its parent can now be destroyed.
        model.instanceRemoved.push_back([this](int32_t id) { editors.erase(id); });
    }

    std::string takeError() {
        return std::exchange(pendingError, {});
    }

    void pump() {
        // Match cat-plugin-player: avoid paint/timer starvation during egui redraws.
        // Leave input, close and WM_QUIT messages to winit's event loop.
        for (const auto& [id, editor] : editors) {
            if (!editor.window) continue;
            MSG message{};
            for (UINT kind : {WM_PAINT, WM_TIMER}) {
                for (int count = 0; count < 32 &&
                     PeekMessageW(&message, static_cast<HWND>(editor.window->getHandle()),
                                  kind, kind, PM_REMOVE); ++count) {
                    if (message.message == WM_QUIT) {
                        PostQuitMessage(static_cast<int>(message.wParam));
                        return;
                    }
                    DispatchMessageW(&message);
                }
            }
        }
    }

    void closeAll() {
        for (auto& [id, editor] : editors) {
            if (auto* plugin = instance(id)) {
                if (editor.created) {
                    plugin->hideUI();
                    plugin->destroyUI();
                    editor.created = false;
                }
            }
            if (editor.window) editor.window->show(false);
        }
        editors.clear();
    }
};
