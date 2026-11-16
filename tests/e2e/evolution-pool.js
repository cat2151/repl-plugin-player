// Test-owned preparation protocol. Timing and fade envelopes are tested in Rust.
// %% only A is loaded before playback and each silent handoff requests one new sound
{
    const source=globalThis.__replCall('snippet.source',{name:'builtin/lib/evolution-pool.js'});
    const entries=[{format:'CLAP',plugin:'Fixture A',pluginId:'a'},
        {format:'CLAP',plugin:'Fixture B',pluginId:'b'}];
    const states=[{state:0,instanceId:-1,loading:false,error:''},
        {state:4,instanceId:-1,loading:false,error:''}];
    const jobs=new Map();
    const requested=[];
    const nativeLoads=[];
    let nextJob=0;
    let tick;
    let closed=false;
    const repl={track:{},processEvents:()=>{
        for (const s of states) if (s.loading) {s.loading=false;s.state=2;}
    },tasks:{every:(name,ms,fn)=>{tick=fn;},cancel:()=>{}}};
    const include=name=>name.endsWith('/plugins.js')?{
        catalogEntry:(format,plugin)=>entries.find(e=>e.plugin===plugin)
    }:name.endsWith('/patch.js')?{
        random:({tracks,load})=>tracks.map(track=>{
            const sound={...entries[requested.length%2],patch:'test-'+requested.length,track};
            requested.push(sound);
            return {...sound,...load(sound)};
        })
    }:{};
    const __replCall=(api,args)=>{
        if (api==='evolution.open') return;
        if (api==='evolution.status') return states.map(s=>({...s}));
        if (api==='evolution.load') {
            e2e.assert(states[args.slot].state===0,'An audible slot was loaded');
            states[args.slot]={state:1,loading:true,instanceId:100+nativeLoads.length,error:''};
            nativeLoads.push(args);
            return;
        }
        if (api==='evolution.close') {closed=true;return;}
        if (api==='patchJob.start') {jobs.set(++nextJob,{state:'completed',file:'fixture-state'});return nextJob;}
        if (api==='patchJob.get') return jobs.get(args.jobId);
        if (api==='patchJob.release') {jobs.delete(args.jobId);return;}
        throw new Error(api);
    };
    const pool=eval(source).prepare([{track:3},{track:4}],entries);
    pool.start({role:'chord',kind:'pad'},1000,{holdMs:5000,fadeMs:1000,everyMs:25});
    e2e.assert(nativeLoads.length===1 && nativeLoads[0].slot===0,'Startup waited for or loaded B');
    e2e.assert(requested.length===1,'B selection began before A playback');
    states[0].state=3; states[1].state=0;
    tick();
    e2e.assert(requested.length===2 && nativeLoads.length===1,'B was not requested immediately');
    tick(); repl.processEvents(); tick();
    e2e.assert(nativeLoads.length===2 && jobs.size===0,'B preparation was not released');
    states[1].state=3; tick();
    e2e.assert(requested.length===2,'Active B was reloaded');
    states[0].state=0; tick();
    e2e.assert(requested.length===3,'Retired A was not requested immediately');
    pool.stop();
    e2e.assert(closed && jobs.size===0,'Exit leaked preparation or native voices');
}
