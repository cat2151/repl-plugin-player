// %% @text name_show_instrument_ui
{
    const uiTrack = 0;
    const instrument = include('builtin/lib/plugins.js').instrumentOn(uiTrack);
    if (instrument < 0) throw new Error(repl.text('show_ui_00', {track:uiTrack}));
    uapmd.instance(instrument).showUI();
}
