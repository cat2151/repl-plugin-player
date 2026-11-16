#pragma once
#include "sequence_dll.hpp"
#include "../patches/uapmd/evolution-graphs/graphs.hpp"
namespace repl_evolution {
struct Api {
    uint32_t version;
    void* (*create)(double,double);
    void (*destroy)(void*);
    uint32_t (*status)(void*,uint32_t);
    float (*gain)(void*,uint32_t);
    bool (*claim)(void*,uint32_t);
    void (*ready)(void*,uint32_t,bool);
    uint32_t (*publish)(void*,const repl_sequence::Plan*);
    bool (*pending)(void*);
    void (*begin)(void*,uint32_t,double);
    bool (*render)(void*,uint32_t,uint32_t,double,repl_sequence::Emit,void*);
    void (*mix)(void*,uint32_t,float*,uint32_t);
    void (*mix64)(void*,uint32_t,double*,uint32_t);
    void (*finish)(void*,uint32_t);
    void (*pulse)(void*);
    bool (*spinning)(void*);
    void (*update_priority)();
    uint32_t (*boosted_threads)();
};
inline const Api* api{};
}
extern "C" __declspec(dllexport) bool repl_set_evolution_api(const repl_evolution::Api* value) {
    if (!value || value->version != 3) return false;
    repl_evolution::api = value;
    repl_evolution_graphs::worker_poll.store(value->update_priority,std::memory_order_release);
    return true;
}
