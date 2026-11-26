#pragma once
#include "../patches/uapmd/clap-plugin-handle/clap_plugin_handle.hpp"
#include "../patches/uapmd/headless-clap-host/headless_scope.hpp"
#include <chrono>
#include <memory>
#include <string>
#include <thread>

// A new instance of a plugin that no track holds, made for one call and
// destroyed after it. No plugin is named here.
namespace repl_fresh_instance {
// Given the new instance's `clap_plugin*`, null when it is not CLAP.
using Use = void (*)(void* user, const void* plugin);

// Main thread. The error, empty when `use` was called.
inline std::string with(uapmd::SequencerEngine& engine, std::string format, std::string plugin, Use use, void* user) {
    // Shared with the creation callback, which may outlive a wait that timed out.
    struct Pending {
        std::unique_ptr<uapmd::PreparedSequencerTrack> track;
        bool done{};
        int32_t id{-1};
        std::string error;
    };
    auto pending = std::make_shared<Pending>();
    pending->track = engine.prepareTrack();
    if (!pending->track) return "Cannot prepare a track for a new instance";
    {
        repl_headless::Scope headless; // nobody observes its parameters
        engine.addPluginToPreparedTrack(*pending->track, format, plugin, [pending](int32_t id, std::string error) {
            pending->id = id;
            pending->error = std::move(error);
            pending->done = true;
        });
    }
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(30);
    while (!pending->done && std::chrono::steady_clock::now() < deadline) {
        remidy::EventLoop::processQueuedTasks();
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
    if (!pending->done) return "The new instance was not made in time";
    auto* instance = pending->id < 0 ? nullptr : pending->track->pluginInstance(pending->id);
    if (!instance) return pending->error.empty() ? std::string("The new instance is unavailable") : pending->error;
    use(user, repl_clap_plugin_handle::of(instance));
    pending->track.reset();
    return {};
}
}
