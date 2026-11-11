// %% @text name_try_preset
// @text comment_06
// @text comment_07
// @text comment_08
var surgePresetQuery = 'Attacky';
var surgePresetChoice = 0;
var surgePresetPreviewOnly = false;
// @text comment_09
(() => {
    var id = globalThis.surgeId;
    var track = include('builtin/lib/plugins.js').trackFor(id, 'snippet_00');
    var synth = uapmd.instance(id);
    var matches = synth.getPresets()
        .filter(p => p.name.toLowerCase().includes(surgePresetQuery.toLowerCase()));
    var candidates = matches.map((p, choice) => ({choice, index:p.index, name:p.name, bank:p.bank}));
    if (surgePresetPreviewOnly) return {count:candidates.length, candidates};
    if (!Number.isInteger(surgePresetChoice) || surgePresetChoice < 0 || surgePresetChoice >= matches.length)
        throw new Error(repl.text('snippet_09'));
    var selected = matches[surgePresetChoice];
    var prior = globalThis.surgePresetRequest;
    var job;
    if (prior) {
        job = uapmd.mutations.getJob(prior.jobId);
        if (!job.state) throw new Error(repl.text('snippet_10'));
        if (job.state !== 'completed' && job.state !== 'failed')
            return {selected:prior.selected, job, presetJobId:job.jobId, message:repl.text('snippet_11')};
    }
    var demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    demo.restoreBypassControls(id);
    job = synth.loadPreset(selected.index);
    globalThis.surgePresetRequest = {instanceId:id, selected, jobId:job.jobId};
    if (job.state === 'failed') throw new Error(repl.text('snippet_12', {error:job.error}));
    uapmd.sequencer.setPluginBypassed(id, false);
    var mml = 'l4 r c r c r c';
    var clip = demo.replaceMml(track.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    return {instanceId:id, trackIndex:track.trackIndex, ...clip, selected, job, presetJobId:job.jobId, candidates,
        message:repl.text(job.state === 'completed' ? 'snippet_13' : 'snippet_11')};
})();
