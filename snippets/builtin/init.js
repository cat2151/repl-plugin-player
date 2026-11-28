// @text comment_init_keys
{
    const scripts = repl.keys.config().scripts;
    for (const key of Object.keys(scripts)) repl.keys.bind(key, () => include(scripts[key]));
}
// @text comment_46
{
    const startTrack = 0;
    include('builtin/lib/plugins.js').ensureInstrument(startTrack, {format:'CLAP', plugin:'Surge XT'});
}
