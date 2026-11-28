#include <cstdint>
#include <vector>
#include "pitch_detection.h"

extern "C" __declspec(dllexport) float repl_mpm_pitch(
    const float* samples, int32_t length, int32_t sample_rate) noexcept {
    if (!samples || length < 3 || sample_rate <= 0)
        return -1.0f;
    try {
        return pitch::mpm(std::vector<float>(samples, samples + length), sample_rate);
    } catch (...) {
        return -1.0f;
    }
}
