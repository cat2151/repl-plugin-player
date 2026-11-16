// Test-owned topology and music; editable snippets are checked only for errors.
// %% ordinary state
globalThis.modeFixture = (() => {
    const tracks = uapmd.sequencer.getTrackInfos();
    const track = 0;
    const id = include('builtin/lib/plugins.js').instrumentOn(track);
    const clip = uapmd.timeline.createEmptyMidiClip(tracks[track].trackId, 0, 480, 120);
    const parameters = uapmd.instance(id).getParameters().map(p=>[p.id,uapmd.instance(id).getParameterValue(p.id)]);
    repl.track.setVolume(track, 0.37);
    repl.timeline.setLoop(100, 20000);
    const check = () => {
        const current = uapmd.sequencer.getTrackInfos();
        if (JSON.stringify(current.map(t=>t.nodes.filter(n=>n.instanceId>=0).map(n=>n.instanceId))) !==
            JSON.stringify(tracks.map(t=>t.nodes.filter(n=>n.instanceId>=0).map(n=>n.instanceId))))
            throw new Error('Ordinary instances changed');
        if (current.length!==tracks.length || !uapmd.timeline.getClips(tracks[track].trackId).some(c=>c.clipId===clip))
            throw new Error('Ordinary tracks or clip changed');
        if (Math.abs(repl.track.volume(track)-0.37)>1e-5 ||
            JSON.stringify(uapmd.instance(id).getParameters().map(p=>[p.id,uapmd.instance(id).getParameterValue(p.id)]))!==JSON.stringify(parameters))
            throw new Error('Ordinary volume or patch changed');
        if (globalThis.evolutionMode || globalThis.evolutionPool || globalThis.evolution || repl.sequence.view)
            throw new Error('Evolution state survived exit');
    };
    return {tracks,id,check};
})();

// %% enter with isolated instances
{
    const mode = include('builtin/lib/evolution-mode.js');
    const slots = Array.from({length:2},()=>({format:'CLAP',plugin:'Surge XT'}));
    const pool = mode.enter(slots);
    if (repl.scope.target() !== -2) throw new Error('Master scope was not selected');
    if (mode.enter(slots)!==pool || pool.buffers.some(b=>b.track<modeFixture.tracks.length))
        throw new Error('Mode was recreated or reused ordinary tracks');
    if (!uapmd.sequencer.isPluginBypassed(modeFixture.id)) throw new Error('Ordinary instrument was not isolated');
    repl.track.setVolume(pool.buffers[0].track,0.2);
    if (globalThis.evolutionPool!==pool) throw new Error('Volume edit ended the mode');
}

// %% native performance and pending state preparation
{
    const pool=globalThis.evolutionPool;
    const controller=include('builtin/lib/evolution.js').start({mode:'sequence',pool,
        sounds:{role:'chord',kind:'pad'},crossfade:{holdMs:5000,fadeMs:1000,everyMs:25},
        progression:'CM7-FM7',inversions:[0,1,2,3],octaves:[-1,0,1],maxLeap:7,
        range:{min:41,max:91,low:60,high:71},bpm:120,beats:4,velocity:18000,everyMs:250});
    e2e.wait(100);
    pool.poll();
}

// %% explicit exit
{
    include('builtin/lib/evolution-mode.js').end();
    include('builtin/lib/evolution-mode.js').end();
    modeFixture.check();
    const state=uapmd.timeline.getState();
    if (!state.loopEnabled || state.loopStartSamples!==100 || state.loopEndSamples!==20000 || state.isPlaying)
        throw new Error('Ordinary loop settings were not restored stopped');
}

// %% ordinary demo leaves the mode
{
    include('builtin/lib/evolution-mode.js').enter([{format:'CLAP',plugin:'Surge XT'},{format:'CLAP',plugin:'Surge XT'}]);
    include('builtin/lib/tutorial.js').reset();
    modeFixture.check();
}

// %% easy reset removes the pool and ordinary plugins
{
    include('builtin/lib/evolution-mode.js').enter([{format:'CLAP',plugin:'Surge XT'},{format:'CLAP',plugin:'Surge XT'}]);
    include('builtin/lib/easy.js').create({format:'CLAP',plugin:'Surge XT',bpm:120}).reset();
    if (globalThis.evolutionMode || uapmd.sequencer.getTrackInfos().length!==modeFixture.tracks.length ||
        uapmd.sequencer.getTrackInfos().some(t=>t.nodes.some(n=>n.instanceId>=0)))
        throw new Error('Easy reset retained evolution or ordinary instances');
}
