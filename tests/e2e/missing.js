// `missing`: a startup cell naming a plugin no catalog has stops before Ready
// with the install message. The check is Rust (`src/e2e.rs`).

// %% init
{
    const startTrack = 0;
    include('builtin/lib/plugins.js').ensureInstrument(startTrack, {format:'CLAP', plugin:'Deliberately Missing Plugin'});
}
