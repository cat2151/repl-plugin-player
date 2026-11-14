#pragma once
#include <cstdint>

namespace repl_loop_end {
// Scoped to one MIDI clip render call on the current audio worker.
inline thread_local bool includeEndNoteOff = false;
struct Scope {
    bool previous = includeEndNoteOff;
    explicit Scope(bool enabled) { includeEndNoteOff = enabled; }
    ~Scope() { includeEndNoteOff = previous; }
};

inline bool isNoteOff(uint32_t first, uint32_t second) {
    const auto type = first >> 28;
    const auto status = (first >> 20) & 15;
    return (type == 2 || type == 4) &&
        (status == 8 || (status == 9 && (type == 2 ? (first & 127) == 0 : (second >> 16) == 0)));
}
}
