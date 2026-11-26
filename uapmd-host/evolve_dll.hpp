#pragma once
#include "sequence_abi.hpp"
#include <cstddef>
#include <cstdint>

// Layout mirrors extension-api/src/evolve.rs; the loader verifies the sizes.
namespace repl_evolve {
using CreateDone = void (*)(void* user, uint64_t handle, int32_t instance, const char* error);
using LoadDone = void (*)(void* user, bool ok, const char* error);
constexpr uint64_t current_generation = UINT64_MAX;
struct HostApi {
    size_t size;
    void* ctx;
    void (*create_instance)(void*, const char*, const char*, CreateDone, void*);
    void (*destroy_instance)(void*, uint64_t);
    void (*load_state)(void*, uint64_t, const uint8_t*, size_t, LoadDone, void*);
    bool (*attach_to_slot)(void*, uint32_t, uint64_t);
    uint64_t (*detach_from_slot)(void*, uint32_t);
};
struct Group {
    const int32_t* tracks;
    uint32_t track_count;
    const char* input;
};
struct Api {
    uint32_t version;
    size_t api_size, host_size, group_size, plan_size;
    void* (*create)(const Group*, uint32_t, double, double, const HostApi*);
    void (*destroy)(void*);
    uint32_t (*slot_count)(void*);
    int32_t (*slot_of)(void*, int32_t);
    uint32_t (*submit)(void*, uint32_t, const char*, const char*, const char*, uint64_t);
    const char* (*poll)(void*);
    uint64_t (*bump_generation)(void*);
    const char* (*status)(void*);
    uint32_t (*publish)(void*, const repl_sequence::Plan*, const int32_t*, uint32_t);
    bool (*pending)(void*);
    bool (*before)(void*, int32_t, uint32_t, double, uint8_t*, size_t*, size_t);
    void (*after)(void*, int32_t, void* const*, uint32_t, uint32_t, bool);
};
inline const Api* api{};
}
extern "C" __declspec(dllexport) bool repl_set_evolve_api(const repl_evolve::Api* value) {
    if (!value || value->version != 3 || value->api_size != sizeof(repl_evolve::Api) ||
        value->host_size != sizeof(repl_evolve::HostApi) || value->group_size != sizeof(repl_evolve::Group) ||
        value->plan_size != sizeof(repl_sequence::Plan))
        return false;
    repl_evolve::api = value;
    return true;
}
