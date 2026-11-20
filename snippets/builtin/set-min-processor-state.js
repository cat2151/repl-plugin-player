// %% @text name_set_min_processor_state
// @search percent power plan 電源プラン 省電力 clock クロック throttling
{
    const percent = 100;
    const restorePercent = 5;
    include('builtin/lib/power.js').setMinProcessorState(percent, restorePercent);
    `${percent}%`;
}
