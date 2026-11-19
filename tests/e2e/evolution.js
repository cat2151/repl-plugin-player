// Test-owned music. Editable snippets are smoke-tested separately.
// %% crossfade endpoints and holds repeat independently of musical loop length
{
    const mix = include('builtin/lib/crossfade.js').mix;
    for (const [ms, expected] of [[0,0],[4999,0],[5000,0],[7500,0.5],[10000,1],
        [14999,1],[15000,1],[17500,0.5],[20000,0],[24999,0],[27500,0.5],[30000,1]])
        e2e.assert(Math.abs(mix(ms,5000,5000)-expected)<1e-9, 'Wrong fade at '+ms);
}
// %% catalog selection retries unreadable patches without reusing a sound
{
    const source = __replCall('snippet.source', {name:'builtin/lib/patch.js'});
    const entry = {format:'CLAP',pluginId:'fixture'};
    const targets = [0,1];
    const loaded = [];
    const failures = [];
    const uapmd = {catalog:{getCount:()=>1,getPluginAt:()=>entry},
        sequencer:{setPluginBypassed:()=>{}}};
    const repl = {
        patchCatalog: ({role,kind}) => {
            e2e.assert(role==='chord' && kind==='pad','Catalog query was lost');
            return ['unreadable','first','second'].map(patch=>({...entry,plugin:'Fixture',patch}));
        },
        patchState: ({patch}) => {
            if (patch==='unreadable') throw new Error('fixture state failure');
            return patch;
        }
    };
    const console = {log:message=>failures.push(message)};
    const include = path => path.endsWith('/plugins.js') ? {
        checkPatch:(entry,patch)=>repl.patchState({patch}),
        loadPatch:(id,created,entry,patch)=>loaded.push([id,repl.patchState({patch})]),
        connectionsOf:()=>[],instrumentOn:track=>track+100
    } :{info:track=>({nodes:[{...entry,instanceId:track+100}]})};
    const library = eval(source);
    const originalRandom = Math.random;
    Math.random = ()=>0;
    try {
        const selected = library.random({tracks:targets,role:'chord',kind:'pad'});
        e2e.assert(e2e.same(selected.map(p=>p.patch),['first','second']),'Failed patch was reused or sounds repeat');
        e2e.assert(e2e.same(loaded,[[100,'first'],[101,'second']]),'Sounds went to the wrong track');
        e2e.assert(failures.length===1 && failures[0].includes('unreadable'),'Failure was hidden');
        const replacement = library.random({tracks:[targets[0]],role:'chord',kind:'pad',exclude:[selected[0]]});
        e2e.assert(replacement[0].patch==='second','Excluded sound was selected again');
        let failed = false;
        try { library.random({tracks:targets,role:'chord',kind:'pad',attempts:1}); }
        catch (error) { failed = error.message.includes('unreadable'); }
        e2e.assert(failed,'Retry limit was ignored');
    } finally { Math.random = originalRandom; }
}

// %% silent buffers refresh once per endpoint and retain a full hold after loading
{
    const source = __replCall('snippet.source', {name:'builtin/lib/crossfade.js'});
    let now = 0;
    let playing = true;
    let tick;
    const levels = [0.8,0.6];
    const changes = [];
    const Date = {now:()=>now};
    const uapmd = {timeline:{getState:()=>({isPlaying:playing})}};
    const repl = {track:{volume:t=>levels[t],setVolume:(t,v)=>{levels[t]=v;}},
        tasks:{every:(name,fnEvery,callback)=>{tick=callback;},cancel:()=>{}}};
    const fade = eval(source).start({tracks:[0,1],holdMs:100,fadeMs:100,everyMs:10,
        onSilent:track=>{
            e2e.assert(levels[track]===0 && levels[1-track]>0,'Sound changed on an audible buffer');
            changes.push(track);
            now += 350; // Simulate a slow load and re-entrant event processing.
            tick();
        }});
    for (const time of [100,150,199]) { now=time; tick(); }
    e2e.assert(changes.length===0,'Sound changed before fade completion');
    now=200; tick();
    e2e.assert(e2e.same(changes,[0]),'First silent buffer was not refreshed once');
    now+=99; tick();
    e2e.assert(levels[0]===0,'Loading consumed the silent hold');
    now+=51; tick();
    e2e.assert(Math.abs(levels[0]-0.4)<1e-9,'Fade did not resume after the hold');
    playing=false; tick(); now+=1000; tick();
    e2e.assert(changes.length===1,'Paused transport refreshed a buffer');
    playing=true; tick();
    now+=1000; tick(); // Skip a whole hold: pin the endpoint before loading.
    e2e.assert(e2e.same(changes,[0,1]),'Second silent buffer was skipped');
    now+=200; tick();
    e2e.assert(e2e.same(changes,[0,1,0]),'Buffers did not alternate');
    fade.stop(); fade.stop();
    e2e.assert(e2e.same(levels,[0.8,0.6]),'Original levels were not restored');
}

// %% chord notation and all inversions satisfy live voice leading
{
    const voice = include('builtin/lib/voicings.js').create('CM7-FM7-Dm7-G7',
        {inversions:[0,1,2,3], octaves:[-3,-2,-1,0,1,2,3], maxLeap:7,
            range:{min:41,max:91,low:60,high:71}});
    const seen = new Set();
    let previous;
    const start = Date.now();
    for (let turn = 0; turn < 80; ++turn) {
        const next = voice.next();
        e2e.assert(next.steps.flat().every(p => p >= 41 && p <= 91), 'Voicing escaped hard pitch bounds');
        e2e.assert(e2e.same(cmrt_chord.note_progression(next.progression).map(c=>c.sort((a,b)=>a-b)),next.steps),
            'Notation and MIDI pitches disagree: ' + next.progression);
        for (const chord of next.progression.split('-')) {
            const match = /\^(\d)/.exec(chord);
            seen.add(match ? Number(match[1]) : 0);
        }
        for (let i = 0; i < next.steps.length; ++i) {
            const j = (i+1)%next.steps.length;
            const pairs = [[next.steps[i],next.steps[j]]];
            if (previous) pairs.push([previous.steps[i],next.steps[j]], [next.steps[i],previous.steps[j]]);
            e2e.assert(pairs.every(([a,b])=>a.every((pitch,v)=>Math.abs(pitch-b[v])<=7)),
                'Leap exceeds seven semitones, including seam/update');
        }
        previous = next;
    }
    e2e.assert([0,1,2,3].every(i=>seen.has(i)), 'An inversion is never used: '+JSON.stringify([...seen]));
    ({iterations:80, milliseconds:Date.now()-start, inversions:[...seen]});
}

// %% weighted randomness favors the central register without a fixed cycle
{
    const random = Math.random;
    let seed = 19841;
    Math.random = () => ((seed = (Math.imul(seed,1664525)+1013904223) >>> 0) + 0.5) / 4294967296;
    try {
        const create = include('builtin/lib/voicings.js').create;
        const options = {inversions:[0],octaves:[0,1,2],maxLeap:127,
            range:{min:0,max:127,low:60,high:71}};
        const voice = create('C', options);
        const counts = new Map();
        const patterns = new Set();
        let last;
        let repeats = 0;
        for (let i = 0; i < 2400; ++i) {
            const pitch = voice.next().steps[0][0];
            counts.set(pitch, (counts.get(pitch)||0)+1);
            if (pitch === last) ++repeats;
            last = pitch;
            if (i < 24) patterns.add(JSON.stringify(create('C', options).next().steps));
        }
        const ordered = [...counts.entries()].sort((a,b)=>a[0]-b[0]);
        e2e.assert(ordered.length === 3, 'Random choice excluded a valid octave');
        e2e.assert(ordered[0][1] > ordered[1][1]*1.2 && ordered[1][1] > ordered[2][1]*2,
            'Distance does not increasingly discourage distant voicings: '+JSON.stringify(ordered));
        e2e.assert(patterns.size > 1 && repeats > 0, 'Choices still follow a fixed cycle');
    } finally { Math.random = random; }
}

// %% start an evolving Rust clock
globalThis.evolutionFixture = {mode:'sequence',track:0, progression:'CM7-FM7-Dm7-G7',
    inversions:[0,1,2,3],octaves:[-1,0,1],maxLeap:7,range:{min:41,max:91,low:60,high:71},
    bpm:300,beats:1,velocity:18000,everyMs:250};
repl.test.loopTrace(true);
uapmd.audio.setEngineEnabled(true);
include('builtin/lib/evolution.js').start(evolutionFixture);
globalThis.initialEvolution = JSON.stringify(evolution.pattern);
e2e.wait(1600);
e2e.assert(JSON.stringify(evolution.pattern)!==initialEvolution,'Periodic JS did not change the pattern');
e2e.assert(e2e.same(repl.sequence.view.steps,evolution.pattern.steps),'Live pattern is absent from the display projection');

// %% JavaScript stalls cannot stall or shift the Rust clock
{
    const end = Date.now()+1000;
    while (Date.now()<end) {}
    const trace = e2e.trace();
    const events = trace.events.filter(e=>e.instance===e2e.instrument());
    const ons = e2e.ons(events);
    e2e.assert(ons.length>20,'Rust clock did not keep playing');
    const period = uapmd.sequencer.getSampleRate()*0.2;
    const origin = ons[0][0];
    e2e.assert(events.every(e=>Math.abs((e.frame-origin)/period-Math.round((e.frame-origin)/period))*period<=3),
        'An update or JS stall moved a note off its audio-clock boundary');
    e2e.assert(e2e.observe(200).peak>0.001,'Evolving source produced no sound');
}

// %% stop
repl.sequence.clear();

// %% two native tracks follow the timed crossfade
{
    const targets = [0,1];
    const demo = include('builtin/lib/tutorial.js');
    const plugins = include('builtin/lib/plugins.js');
    const start = include('builtin/lib/evolution.js').start;
    demo.reset();
    while (uapmd.sequencer.getTrackInfos().length <= Math.max(...targets)) uapmd.sequencer.addTrack();
    for (const track of targets) {
        plugins.ensureInstrument(track, {format:'CLAP',plugin:'Surge XT'});
        uapmd.sequencer.setTrackFreezePolicy(e2e.trackId(track), 'off');
    }
    for (const mode of ['sequence']) {
        demo.reset();
        for (const track of targets) {
            uapmd.sequencer.setPluginBypassed(plugins.instrumentOn(track),false);
            repl.track.setVolume(track,1);
        }
        repl.test.loopTrace(true);
        uapmd.audio.setEngineEnabled(true);
        start({...evolutionFixture,mode,tracks:targets,beats:4,velocity:mode==='sequence'?18000:6,
            crossfade:{holdMs:5000,fadeMs:5000,everyMs:25}});
        const begun = Date.now();
        for (const [ms, blend] of [[1000,0],[4500,0],[7500,0.5],[11000,1],[14500,1],[17500,0.5],[20500,0]]) {
            e2e.wait(Math.max(0,begun+ms-Date.now()));
            const levels = targets.map(track=>repl.track.volume(track));
            e2e.assert(Math.abs(levels[0]-(1-blend))<0.1 && Math.abs(levels[1]-blend)<0.1,
                mode+' wrong fade at '+ms+': '+JSON.stringify(levels));
            e2e.assert(e2e.observe(50).peak>0.001,mode+' crossfade went silent at '+ms);
        }
        const events = e2e.trace().events;
        for (const track of targets)
            e2e.assert(e2e.ons(events.filter(e=>e.instance===plugins.instrumentOn(track))).length>4,
                mode+' did not play track '+track);
        evolution.stop();
        e2e.assert(targets.every(track=>Math.abs(repl.track.volume(track)-1)<1e-6),'Fade levels were not restored');
    }
    demo.reset();
}

// %% catalog sounds rotate through silent buffers in native playback
{
    const targets = [0,1];
    const demo = include('builtin/lib/tutorial.js');
    const key = p => JSON.stringify([p.format,p.pluginId,p.patch]);
    for (const mode of ['sequence']) {
        demo.reset();
        uapmd.audio.setEngineEnabled(true);
        const controller = include('builtin/lib/evolution.js').start({...evolutionFixture,
            mode,tracks:targets,beats:4,bpm:300,velocity:mode==='sequence'?18000:6,
            sounds:{role:'chord',kind:'pad'},crossfade:{holdMs:2000,fadeMs:500,everyMs:25}});
        let before = controller.sounds.map(key);
        let changes = 0;
        const deadline = Date.now()+30000;
        while (changes < 3 && Date.now() < deadline) {
            e2e.wait(100);
            const after = controller.sounds.map(key);
            if (e2e.same(after,before)) continue;
            const index = changes%2;
            e2e.assert(after[index]!==before[index] && after[index]!==before[1-index]
                && after[1-index]===before[1-index],mode+' replaced the wrong sound');
            e2e.assert(repl.track.volume(targets[index])===0,mode+' replaced an audible sound');
            before = after;
            ++changes;
        }
        e2e.assert(changes===3,mode+' stopped rotating sounds');
        controller.stop();
    }
    demo.reset();
}

// %% stop
repl.sequence.clear();
