// %% @text name_try_play
// @text demo_reset
{
    include('builtin/lib/evolution-mode.js').end();
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.init({instanceId:include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', track:synthTrack}).instanceId, bpm:120});
    demo.play('cfg');
}
