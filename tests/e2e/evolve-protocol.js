// Test-owned request protocol of evolve-pool.js over a mock native side. Loading,
// pooling and fades are tested in Rust and by evolve-host.
// %% requests return at once, the same and another kind are submitted alike, stale outcomes are ignored, and dispose releases a preparing job
{
    const source=globalThis.__replCall('snippet.source',{name:'builtin/lib/evolve-pool.js'});
    const entries=[{format:'CLAP',plugin:'Fixture A',pluginId:'a'},
        {format:'CLAP',plugin:'Fixture B',pluginId:'b'}];
    const states=[0,0];
    const jobs=new Map();
    const submits=[];
    const outcomes=[];
    const calls=[];
    const picks=[];
    let nextJob=0;
    let nextRequest=0;
    let generation=0;
    let tick;
    const repl={track:{},processEvents:()=>{throw new Error('evolve-pool.js waited for a load');},
        tasks:{every:(name,ms,fn)=>{tick=fn;},cancel:()=>{}}};
    const include=name=>name.endsWith('/plugins.js')?{
        catalogEntry:(format,plugin)=>entries.find(e=>e.plugin===plugin)
    }:name.endsWith('/patch.js')?{
        random:({tracks,load})=>tracks.map(track=>{
            const sound={...entries[picks.shift() ?? 0],patch:'test-'+nextJob,track};
            return {...sound,...load(sound)};
        })
    }:{};
    const __replCall=(api,args)=>{
        calls.push(api);
        if (api==='evolve.open' || api==='evolve.close') return;
        if (api==='evolve.status') return states.map(state=>({state,instanceId:-1}));
        if (api==='evolve.submit') {
            e2e.assert(states[args.slot]===0,'A sounding slot was requested');
            submits.push({...args,requestId:++nextRequest,generation});
            return nextRequest;
        }
        if (api==='evolve.poll') return outcomes.splice(0);
        if (api==='evolve.bumpGeneration') return ++generation;
        if (api==='patchJob.start') {jobs.set(++nextJob,{state:'pending'});return nextJob;}
        if (api==='patchJob.get') return jobs.get(args.jobId);
        if (api==='patchJob.release') {jobs.delete(args.jobId);return;}
        throw new Error(api);
    };
    const finish=()=>{for (const job of jobs.values()) Object.assign(job,{state:'completed',file:'fixture-state'});};
    const settle=(request,{stale=false,error=''}={})=>
        outcomes.push({requestId:request.requestId,slot:request.slot,elapsedMs:1,stale,error});
    const count=api=>calls.filter(c=>c===api).length;
    const pool=eval(source).prepare([{track:3},{track:4}],entries);

    picks.push(0,1);
    pool.start({role:'chord',kind:'pad'},1000,{holdMs:5000,fadeMs:1000,everyMs:25});
    e2e.assert(count('evolve.open')===1 && jobs.size===2 && submits.length===0,
        'Start waited for a preparation: '+JSON.stringify(calls));
    tick();
    e2e.assert(submits.length===0 && jobs.size===2,'A pending preparation was submitted or requested again');
    finish(); tick();
    e2e.assert(submits.length===2 && jobs.size===0,'Prepared states were not submitted and released');
    e2e.assert(submits[0].slot===0 && submits[0].pluginId==='a' && submits[1].slot===1 && submits[1].pluginId==='b',
        'Submit did not carry the selected sound: '+JSON.stringify(submits));
    tick();
    e2e.assert(count('patchJob.start')===2,'An unsettled slot was requested again');
    settle(submits[0]); settle(submits[1]); states[0]=2; states[1]=2; tick();
    e2e.assert(count('patchJob.start')===2,'A ready slot was requested again');

    // The same kind and another kind go through the same submit; the native side tells them apart.
    states[0]=0; picks.push(0); tick(); finish(); tick();
    states[1]=0; picks.push(1); tick(); finish(); tick();
    e2e.assert(submits.length===4 && submits[2].slot===0 && submits[2].pluginId==='a'
        && submits[3].slot===1 && submits[3].pluginId==='b', 'A retired slot was not requested: '+JSON.stringify(submits));
    e2e.assert(count('evolve.open')===1 && count('evolve.close')===0,'A sound change reopened evolve');

    // Re-running the edit cell drops this cycle's requests without closing evolve.
    const old=submits[2];
    pool.stop();
    pool.start({role:'chord',kind:'pad'},1000,{holdMs:5000,fadeMs:1000,everyMs:25});
    e2e.assert(generation===count('evolve.bumpGeneration') && generation>0,'Stop did not bump the generation');
    e2e.assert(count('evolve.open')===1 && count('evolve.close')===0,'Restart reopened evolve');
    finish(); tick();
    const fresh=submits.filter(s=>s.generation===generation);
    e2e.assert(fresh.length===2,'Free slots were not requested again: '+JSON.stringify(submits));
    const started=count('patchJob.start');
    settle(old,{stale:true}); settle(submits[3],{stale:true,error:'superseded'}); tick();
    e2e.assert(count('patchJob.start')===started && submits.length===6,'A stale outcome was applied');
    settle(fresh[0],{stale:true}); tick();
    e2e.assert(count('patchJob.start')===started+1,'A stale outcome of the current request did not free its slot');

    pool.dispose();
    e2e.assert(count('evolve.close')===1 && jobs.size===0,'Dispose leaked a preparing job');
}
