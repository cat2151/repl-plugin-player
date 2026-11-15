#include "FrozenTrackAudio.hpp"
#include <algorithm>
#include <array>
#include <chrono>
#include <filesystem>
#include <iostream>
#include <stdexcept>
#include <vector>

static void require(bool ok, const std::string& message) {
    if (!ok) throw std::runtime_error(message);
}

static void fill(uapmd::FrozenTrackAudio& audio) {
    int calls = 0;
    while (audio.loadAhead()) require(++calls < 1000, "loader did not settle");
}

static void compare(uapmd::FrozenTrackAudio& audio, int64_t position) {
    for (uint32_t channel = 0; channel < 2; ++channel) {
        std::array<float, 1024> realtime{}, offline{};
        audio.read(channel, position, realtime.data(), realtime.size(), false);
        audio.read(channel, position, offline.data(), offline.size(), true);
        require(std::any_of(offline.begin(), offline.end(), [](float sample) { return sample != 0; }),
                "fixture has no signal");
        require(realtime == offline, "loop-head audio missing at " + std::to_string(position));
    }
}

int main() {
    auto directory = std::filesystem::temp_directory_path() /
        ("repl-prefetch-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    try {
        std::filesystem::create_directories(directory);
        std::string error;
        auto audio = uapmd::FrozenTrackAudio::create(directory, error);
        require(bool(audio), error);
        std::array<float, 8192> left{}, right{};
        const float* channels[]{left.data(), right.data()};
        for (int chunk = 0; chunk < 80; ++chunk) {
            for (int i = 0; i < 8192; ++i) {
                left[i] = static_cast<float>(1 + (chunk * 8192 + i) % 997) / 1000;
                right[i] = -left[i];
            }
            require(audio->writeFrames(channels, 2, 8192, error), error);
        }
        uapmd::FrozenTrackAudioIdentity identity;
        identity.sampleRate = 48000;
        require(audio->finishWriting(0, {1, 1}, {-37, 901}, identity, error), error);
        // Longer than the loader window; fractional chunk boundaries and channel shifts.
        for (int64_t head : {int64_t(0), int64_t(96123), int64_t(193337)}) {
#ifndef PREFETCH_BASELINE
            audio->playbackLoopStart(head);
#endif
            audio->playbackPosition(head);
            fill(*audio);
            const int64_t end = head + 240137;
            for (int pass = 0; pass < 3; ++pass) {
                for (int64_t pos = head; pos < end; pos += 1024) {
                    audio->playbackPosition(pos);
                    fill(*audio);
                }
                // No loader call at the wrap: the callback cannot wait for disk.
                compare(*audio, head);
                compare(*audio, head + 1024);
                audio->playbackPosition(head);
                fill(*audio);
            }
        }
#ifndef PREFETCH_BASELINE
        audio->playbackLoopStart(-1);
#endif
        audio->playbackPosition(500000);
        fill(*audio);
        compare(*audio, 500000);
        audio.reset();
        std::filesystem::remove_all(directory);
        std::cout << "PASS: exact stereo loop-head samples, repeated zero/nonzero wraps, shifts, loop changes, disabled loop\n";
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        std::filesystem::remove_all(directory);
        return 1;
    }
}
