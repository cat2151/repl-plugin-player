#pragma once

namespace repl_headless {
// While positive, CLAP instances created on this thread get HeadlessCLAPHost.
inline thread_local int depth = 0;

// Marks the instances created during its lifetime as having no observer of
// their parameter list or values.
struct Scope {
    Scope() { ++depth; }
    ~Scope() { --depth; }
    Scope(const Scope&) = delete;
    Scope& operator=(const Scope&) = delete;
};
}
