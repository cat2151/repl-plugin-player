#pragma once
#include <uapmd-plugin-hosting/uapmd-plugin-hosting.hpp>

// The generated graph observes the selected instrument; the realtime dispatcher
// observes final master output with instance -2. -1 disables capture. The callback
// is installed before creation and remains valid until engine teardown.
void repl_capture_scope(int32_t instance, uapmd::AudioProcessContext& process) noexcept;
