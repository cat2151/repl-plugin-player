// `dock`: the editor pane in its own window runs the editor. The checks are Rust
// (`src/gui/dock/regression.rs`); it runs these cells by title.

// %% run
// The whole editor text while the editor pane is floating.
globalThis.dockRuns = (globalThis.dockRuns ?? 0) + 1;

// %% runs
globalThis.dockRuns ?? 0
