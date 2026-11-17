#pragma once
#include <cstddef>
#include <cstdint>

// UAPMD adapter only. State and note accounting live in Rust.
void* repl_create_note_counter(int32_t instance);
void repl_destroy_note_counter(void* state);
void repl_process_note_counter(void* state, const uint32_t* words, size_t count);
