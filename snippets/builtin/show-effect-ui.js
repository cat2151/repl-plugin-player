// %% @text name_show_effect_ui
{
    const uiTrack = 0;
    const effects = include('builtin/lib/plugins.js').effectsOn(uiTrack);
    if (effects.length === 0) throw new Error(repl.text('show_ui_01', {track:uiTrack}));
    for (const effect of effects) uapmd.instance(effect).showUI();
}
