// %% @text name_try_preset
// @text comment_06
// @text comment_07
// @text comment_08
{
    const presetTrack = 0;
    const query = 'Attacky';
    const choice = 0;
    const previewOnly = false;
    // @text comment_09
    (() => {
        const id = include('builtin/lib/patch.js').use({format:'CLAP', plugin:'Surge XT', track:presetTrack}).instanceId;
        if (id < 0) throw new Error(repl.text('snippet_00'));
        const synth = uapmd.instance(id);
        const matches = synth.getPresets()
            .filter(p => p.name.toLowerCase().includes(query.toLowerCase()));
        const candidates = matches.map((p, i) => ({choice:i, index:p.index, name:p.name, bank:p.bank}));
        if (previewOnly) return {count:candidates.length, candidates};
        if (!Number.isInteger(choice) || choice < 0 || choice >= matches.length)
            throw new Error(repl.text('snippet_09'));
        const selected = matches[choice];
        const prior = globalThis.surgePresetRequest;
        let job;
        if (prior) {
            job = uapmd.mutations.getJob(prior.jobId);
            if (!job.state) throw new Error(repl.text('snippet_10'));
            if (job.state !== 'completed' && job.state !== 'failed')
                return {selected:prior.selected, job, presetJobId:job.jobId, message:repl.text('snippet_11')};
        }
        const demo = include('builtin/lib/tutorial.js');
        demo.reset();
        // @text demo_levels
        demo.restoreBypassControls(id);
        job = synth.loadPreset(selected.index);
        globalThis.surgePresetRequest = {selected, jobId:job.jobId};
        if (job.state === 'failed') throw new Error(repl.text('snippet_12', {error:job.error}));
        uapmd.sequencer.setPluginBypassed(id, false);
        const mml = 'l4 r c r c r c';
        const clip = demo.replaceMml(presetTrack, mml, 120);
        uapmd.audio.setEngineEnabled(true);
        uapmd.sequencer.startPlayback();
        return {instanceId:id, trackIndex:presetTrack, ...clip, selected, job, presetJobId:job.jobId, candidates,
            message:repl.text(job.state === 'completed' ? 'snippet_13' : 'snippet_11')};
    })();
}
