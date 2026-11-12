// `snippets`: user snippets inserted into the editor run exactly once, alone.
// The checks are Rust (`src/snippets_e2e.rs`): it takes the snippets' code
// from the cells below by title, and inserts them into the editor text of the
// `editor text` and `neighbor` cells, which must never run.

// %% editor text
throw new Error('old cell must not run');

// %% neighbor
throw new Error('neighbor must not run');

// %% add clip
uapmd.timeline.createEmptyMidiClip(0, 0, 480, 120);

// %% failing snippet
globalThis.snippetEffect = 42; throw new Error('snippet error');

// %% recovery
snippetEffect += 1;

// %% effect
snippetEffect
