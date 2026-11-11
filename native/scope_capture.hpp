#pragma once
#include <uapmd-midi-service/uapmd-midi-service.hpp>

// The generated graph observes only the prepared instrument's output. The
// callback is installed before creation and remains valid until engine teardown.
void repl_capture_scope(int32_t instance, uapmd::AudioProcessContext& process) noexcept;
