// Fixed states and four-note input; never reads snippet music.
// %% catalog
() => {
    const names=['Surge XT','Vaporizer2','TyrellN6','Six Sines'];
    const plugins=include('builtin/lib/plugins.js');
    const patches=repl.patchCatalog({role:'chord',kind:'pad'});
    return names.map(plugin=>{
        const entry=plugins.catalogEntry('CLAP',plugin);
        const patch=patches.filter(p=>p.format===entry.format && p.pluginId===entry.pluginId)
            .sort((a,b)=>a.patch<b.patch?-1:a.patch>b.patch?1:0)[0];
        if (!patch) throw new Error('No fixed pad for '+plugin);
        return {format:entry.format,pluginId:entry.pluginId,plugin,patch:patch.patch};
    });
}

// %% create
({sounds,workers,evolve,fadeMs=1000}) => {
    include('builtin/lib/tutorial.js').stop();
    globalThis.fixedFixture={sounds,evolve};
    const fixture=globalThis.fixedFixture;
    __remidy_configure_audio_workers(workers);
    if (evolve) {
        const pool=include('builtin/lib/evolution-mode.js').enter(
            sounds.map(s=>({format:s.format,plugin:s.plugin})));
        fixture.tracks=pool.buffers.map(b=>b.track);
        __replCall('evolution.open',{tracks:fixture.tracks,holdMs:5000,fadeMs});
    } else {
        const plugins=include('builtin/lib/plugins.js');
        for (const track of uapmd.sequencer.getTrackInfos())
            for (const node of track.nodes.filter(n=>n.instanceId>=0))
                uapmd.sequencer.setPluginBypassed(node.instanceId,true);
        fixture.tracks=[0,1];
        while (uapmd.sequencer.getTrackInfos().length<2) {
            const job=uapmd.sequencer.addTrack();
            while (uapmd.mutations.getJob(job.jobId).state==='pending') repl.processEvents();
        }
        fixture.ids=sounds.slice(0,2).map((s,index)=>{
            const id=include('builtin/lib/patch.js').use({...s,track:index}).instanceId;
            plugins.loadState(id,s.file);
            repl.track.setVolume(index,0.2);
            return id;
        });
    }
    return fixture.tracks;
}

// %% start
{
    const f=fixedFixture;
    repl.test.loopTrace(true);
    repl.sequence.set({track:f.tracks[0],tracks:f.tracks,steps:[[60,64,67,71]],
        bpm:120,beats:4,gate:1,velocity:18000});
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}

// %% observe
({signal:repl.test.observe(),diagnostics:__remidy_audio_worker_diagnostics(),
    slots:fixedFixture.evolve?__replCall('evolution.status',{}):[]})

// %% signal
repl.test.observe()

// %% trace
repl.test.loopTrace()

// %% cleanup
{
    include('builtin/lib/tutorial.js').stop();
    if (fixedFixture.evolve) {
        const deadline=Date.now()+120000;
        while (__replCall('evolution.status',{}).some(s=>s.loading)) {
            if (Date.now()>deadline) throw new Error('Load did not finish');
            repl.processEvents();
        }
        __replCall('evolution.close',{});
        include('builtin/lib/evolution-mode.js').end();
    }
    repl.sequence.clear();
    delete globalThis.fixedFixture;
}

// %% status
__replCall('evolution.status',{})

// %% load
({slot,sound}) => __replCall('evolution.load',{slot,format:sound.format,pluginId:sound.pluginId,file:sound.file})

// %% pause
uapmd.sequencer.pausePlayback();

// %% resume
uapmd.sequencer.resumePlayback();

// %% disable audio
uapmd.audio.setEngineEnabled(false);

// %% enable audio
uapmd.audio.setEngineEnabled(true);
if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
