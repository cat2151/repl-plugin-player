// %% @text name_try_play
// @text demo_reset
{
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.init({instanceId:include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', track:synthTrack}).instanceId, preset:'Init Saw', bpm:120});
    demo.play('cfg');
}
