#pragma once
#include "../patches/uapmd/clap-plugin-handle/clap_plugin_handle.hpp"
#include <array>
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

// Puts a patch the Rust side prepared into a live instance. What the prepared
// bytes hold and how each plugin takes its patch is decided in Rust; no plugin
// is named here.
namespace repl_patch_into {
// Receives the state the caller loads into the instance next.
using State = void (*)(void* user, const uint8_t* state, size_t size);
// (plugin, prepared, size, fresh, state, user, error, capacity) -> false with a
// NUL-terminated `error`. Called on the main thread.
using Into = bool (*)(const void*, const uint8_t*, size_t, bool, State, void*, char*, size_t);
inline Into function{};

struct Result {
    std::string error;
    // Empty when the patch went into the instance directly.
    std::vector<uint8_t> state;
};

// Main thread, after the instance was made and before it plays. `fresh` tells
// that nothing was loaded into the instance yet.
inline Result into(uapmd_plugin_hosting::AudioPluginInstanceAPI* instance, const std::vector<uint8_t>& prepared,
                   bool fresh) {
    Result result;
    if (!function) { result.error = "No patch loader is registered"; return result; }
    const void* plugin = instance ? repl_clap_plugin_handle::of(instance) : nullptr;
    const State keep = [](void* user, const uint8_t* state, size_t size) {
        static_cast<std::vector<uint8_t>*>(user)->assign(state, state + size);
    };
    std::array<char, 256> error{};
    if (!function(plugin, prepared.data(), prepared.size(), fresh, keep, &result.state, error.data(), error.size()))
        result.error = error[0] ? std::string(error.data()) : std::string("The patch was not loaded");
    return result;
}
}
