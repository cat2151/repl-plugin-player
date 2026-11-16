#pragma once
#include <array>
#include <cstdint>
#include <stdexcept>

// Layout mirrors extension-api/src/sequence.rs; the loader verifies ABI sizes.
namespace repl_sequence {
struct Step { std::array<uint8_t, 8> notes{}; uint8_t count{}; };
struct Plan {
    std::array<Step, 64> steps{};
    uint32_t count{};
    int32_t track{};
    uint16_t velocity{};
    double seconds{}, gate{};
};
using Emit = void (*)(void*, uint32_t, uint8_t, bool, uint16_t);
struct Api {
    uint32_t version;
    size_t plan_size, step_size;
    void* (*create)();
    void (*destroy)(void*);
    uint32_t (*publish)(void*, const Plan*);
    bool (*pending)(void*);
    void (*reset)(void*);
    void (*rewind)(void*);
    void (*held)(void*, Step*);
    void (*render)(void*, int32_t, uint32_t, double, bool, Emit, void*);
    void (*release)(void*);
};
inline const Api* api{};
class Clock {
    void* handle;
public:
    Clock() : handle(api ? api->create() : nullptr) {
        if (!handle) throw std::runtime_error("Rust sequence DLL is not loaded");
    }
    ~Clock() { api->destroy(handle); }
    Clock(const Clock&) = delete;
    Clock& operator=(const Clock&) = delete;
    Step heldNotes() const { Step out; api->held(handle, &out); return out; }
    bool publish(const Plan& plan) {
        const auto result = api->publish(handle, &plan);
        if (result == 1) throw std::runtime_error("Invalid sequence plan");
        return result == 0;
    }
    void reset() { api->reset(handle); }
    bool pending() const { return api->pending(handle); }
    void rewind() { api->rewind(handle); }
    void release() { api->release(handle); }
    template<class F> void render(int32_t track, uint32_t frames, double rate, bool enabled, F emit) {
        api->render(handle, track, frames, rate, enabled,
            [](void* user, uint32_t at, uint8_t pitch, bool on, uint16_t velocity) {
                (*static_cast<F*>(user))(at, pitch, on, velocity);
            }, &emit);
    }
};
}

extern "C" __declspec(dllexport) bool repl_set_sequence_api(const repl_sequence::Api* api) {
    if (!api || api->version != 3 || api->plan_size != sizeof(repl_sequence::Plan) || api->step_size != sizeof(repl_sequence::Step)) return false;
    repl_sequence::api = api;
    return true;
}
