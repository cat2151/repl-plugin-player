// `tutorials`: every try-* snippet runs twice without error, even after another
// plugin replaced track 0's instrument. The run is Rust (`src/tutorials_e2e.rs`).

// %% other instrument
// Test-owned choice of a plugin that no snippet's preset belongs to.
{
    const otherTrack = 0;
    include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Dexed', track:otherTrack});
}

// %% job
({job}) => uapmd.mutations.getJob(job)
