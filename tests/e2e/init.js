// The E2E startup cell: cases rely on Surge XT, while builtin/init.js stays the user's to edit.
{
    const startTrack = 0;
    include('builtin/lib/plugins.js').ensureInstrument(startTrack, {format:'CLAP', plugin:'Surge XT'});
}
