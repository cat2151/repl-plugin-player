// Test-owned evolve topology and music. Rust checks the final scope waveform.
// %% play evolve master
{
    globalThis.masterScopeBefore = repl.scope.target();
    const pool = include('builtin/lib/evolution-mode.js').enter(
        [{format:'CLAP',plugin:'Surge XT'}, {format:'CLAP',plugin:'Surge XT'}]);
    include('builtin/lib/evolution.js').start({mode:'sequence',pool,
        sounds:{role:'chord',kind:'pad'},crossfade:{holdMs:5000,fadeMs:1000,everyMs:25},
        progression:'CM7-FM7',inversions:[0,1,2,3],octaves:[-1,0,1],maxLeap:7,
        range:{min:41,max:91,low:60,high:71},bpm:120,beats:4,velocity:24000,everyMs:250});
    if (repl.scope.target() !== -2) throw new Error('Evolve lost master scope');
}

// %% restore scope
{
    include('builtin/lib/evolution-mode.js').end();
    if (repl.scope.target() !== globalThis.masterScopeBefore)
        throw new Error('Evolve did not restore scope');
    delete globalThis.masterScopeBefore;
}
