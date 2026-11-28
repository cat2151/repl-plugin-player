// `keys`: the key of `[scripts]` that runs builtin/play-loop.js starts the loop whether
// the editor has focus or not. The checks are Rust (`src/gui/dock/key_regression.rs`);
// it runs these cells by title.

// %% init
// The app's own startup, which binds the keys of `[scripts]`.
include('builtin/init.js')

// %% clip
// One clip on track 0, so that the loop has notes.
{
    const midi = include('builtin/lib/midi.js');
    midi.addFile(0, midi.writeMml('l4 >cg<cg'));
}

// %% key
// The key bound to builtin/play-loop.js, or '' when keybinds.toml has none.
(Object.entries(repl.keys.config().scripts)
    .find(([, path]) => path === 'builtin/play-loop.js') ?? [''])[0]

// %% stop
uapmd.sequencer.stopPlayback();

// %% playing
uapmd.timeline.getState().isPlaying
