#pragma once
#include "sequence_dll.hpp"
#include <cstddef>
#include <cstdint>

// Layout mirrors extension-api/src/evolve2.rs; the loader verifies the sizes.
namespace repl_evolve2 {
using CreateDone = void (*)(void* user, uint64_t handle, const char* error);
using LoadDone = void (*)(void* user, bool ok, const char* error);
struct HostApi {
    size_t size;
    void* ctx;
    void (*create_instance)(void*, const char*, const char*, CreateDone, void*);
    void (*destroy_instance)(void*, uint64_t);
    void (*load_state)(void*, uint64_t, const uint8_t*, size_t, LoadDone, void*);
    bool (*attach_to_slot)(void*, uint32_t, uint64_t);
    uint64_t (*detach_from_slot)(void*, uint32_t);
};
struct Outcome {
    uint32_t request_id;
    uint32_t slot;
    double elapsed_ms;
    bool stale;
    char error[256];
};
struct SlotStatus {
    uint32_t state;
    float gain;
    uint64_t instance;
    char format[16];
    char plugin_id[256];
};
struct Api {
    uint32_t version;
    size_t api_size, host_size, outcome_size, status_size, plan_size;
    void* (*create)(double, double, const HostApi*);
    void (*destroy)(void*);
    uint32_t (*submit)(void*, uint32_t, const char*, const char*, const uint8_t*, size_t, uint64_t);
    uint32_t (*poll)(void*, Outcome*, uint32_t);
    uint64_t (*bump_generation)(void*);
    bool (*status)(void*, uint32_t, SlotStatus*);
    uint32_t (*publish)(void*, const repl_sequence::Plan*);
    bool (*pending)(void*);
    void (*begin)(void*, uint32_t, double);
    bool (*render)(void*, uint32_t, uint32_t, double, repl_sequence::Emit, void*);
    void (*mix)(void*, uint32_t, float*, uint32_t);
    void (*mix64)(void*, uint32_t, double*, uint32_t);
    void (*finish)(void*, uint32_t);
};
inline const Api* api{};
}
extern "C" __declspec(dllexport) bool repl_set_evolve2_api(const repl_evolve2::Api* value) {
    if (!value || value->version != 1 || value->api_size != sizeof(repl_evolve2::Api) ||
        value->host_size != sizeof(repl_evolve2::HostApi) || value->outcome_size != sizeof(repl_evolve2::Outcome) ||
        value->status_size != sizeof(repl_evolve2::SlotStatus) || value->plan_size != sizeof(repl_sequence::Plan))
        return false;
    repl_evolve2::api = value;
    return true;
}
