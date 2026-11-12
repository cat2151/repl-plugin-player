// `patch-browse`: the patch browser flow against a stand-in player. The checks
// and the stand-in are Rust (`src/patch_browse_e2e.rs`); it runs these cells
// by title. Test-owned script: editable snippet contents are never expectations.
// `written cell` is not run: it is the text the browser must write for the
// last patch, with PLUGIN and PATCH replaced by the plugin and the patch.

// %% loop
{
    const loopTrack = 0;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    playLoopParts({main:prepareLoopPart(loopTrack, 'cegb')}, 120);
}

// %% probe
// Before the browse cell: records track 0's plugins, its instrument's
// parameters and the output peak, and wraps `answer` to record, at each
// answer, the plugins, the audition's parameters and the peak.
(() => {
    const wait = ms => { const end = Date.now() + ms; while (Date.now() < end) repl.processEvents(); };
    const peak = () => {
        let high = 0;
        for (let i = 0; i < 10; i++) {
            for (const v of uapmd.sequencer.getOutputTimeDomainData(256)) high = Math.max(high, Math.abs(v));
            wait(30);
        }
        return high;
    };
    const plugins = t => uapmd.sequencer.getTrackInfos().find(x => x.trackIndex === t).nodes
        .filter(n => n.instanceId >= 0)
        .map(n => ({instanceId:n.instanceId, pluginId:n.pluginId, bypassed:uapmd.sequencer.isPluginBypassed(n.instanceId)}));
    const parameters = id => { const i = uapmd.instance(id); return Object.fromEntries(i.getParameters().map(p => [p.id, i.getParameterValue(p.id)])); };
    const connections = t => include('builtin/lib/plugins.js').connectionsOf(t)
        .map(c => [c.busType, c.source.instanceId, c.source.busIndex, c.target.instanceId, c.target.busIndex].join(' ')).sort();
    const track = 0;
    const original = include('builtin/lib/plugins.js').instrumentOn(track);
    const record = {track, original, answers:[], before:{plugins:plugins(track),
        parameters:parameters(original), connections:connections(track), peak:peak()}};
    const answer = repl.patchBrowser.answer;
    repl.patchBrowser.answer = function(error) {
        wait(800);
        const now = plugins(track);
        const audition = now.find(p => p.instanceId !== original);
        record.answers.push({error:error === undefined ? null : error, plugins:now,
            parameters:audition ? parameters(audition.instanceId) : {}, peak:peak()});
        return answer(error);
    };
    globalThis.patchBrowseProbe = {record, answer, wait, peak, plugins, parameters, connections};
    return true;
})()

// %% browse
{
    const track = 0;
    include('builtin/lib/patch.js').browse({track});
}

// %% result
// After the browse cell: restores `answer` and records the track once more.
(() => {
    const p = globalThis.patchBrowseProbe;
    delete globalThis.patchBrowseProbe;
    repl.patchBrowser.answer = p.answer;
    p.wait(800);
    const r = p.record;
    r.after = {plugins:p.plugins(r.track), parameters:p.parameters(r.original),
        connections:p.connections(r.track), peak:p.peak(), playing:uapmd.timeline.getState().isPlaying};
    return r;
})()

// %% written cell
{
    const track = 0;
    include('builtin/lib/patch.js').use({format:'CLAP', plugin:'PLUGIN', patch:'PATCH', track});
}

// %% plugins on track 0
uapmd.sequencer.getTrackInfos().find(t => t.trackIndex === 0).nodes.filter(n => n.instanceId >= 0).map(n => [n.instanceId, n.pluginId])

// %% parameters
({instance}) => { const i = uapmd.instance(instance); return Object.fromEntries(i.getParameters().map(p => [p.id, i.getParameterValue(p.id)])); }
