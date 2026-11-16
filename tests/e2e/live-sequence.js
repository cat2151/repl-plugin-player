// Test-owned patterns; editable built-in music is checked only for execution errors.
// %% arm the native sample clock
globalThis.sequenceTest = {track:0, bpm:137, beats:0.25, chords:[[60,64],[67,71],[64,69]]};
e2e.stopped();
repl.sequence.clear();
repl.timeline.clearLoop();
repl.test.loopTrace(true);
repl.sequence.set({track:sequenceTest.track, steps:sequenceTest.chords,
    bpm:sequenceTest.bpm, beats:sequenceTest.beats, gate:0.7, velocity:18000});
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% block JavaScript for many audio blocks
{
    // Deliberately do not call processEvents, poll, or send notes while waiting.
    const until = Date.now() + 1800;
    while (Date.now() < until) {}
    const trace = e2e.trace();
    const ons = e2e.ons(trace.events, e2e.instrument());
    e2e.assert(ons.length >= 24 && !trace.overflow, 'Native sequence stalled with JS');
    const period = uapmd.sequencer.getSampleRate()*60/sequenceTest.bpm*sequenceTest.beats;
    sequenceTest.period = period;
    sequenceTest.origin = ons[0][0];
    ons.forEach(([frame,pitch],i) => e2e.assert(pitch === sequenceTest.chords[Math.floor(i/2)%3][i%2] &&
        Math.abs(frame-ons[0][0]-Math.floor(i/2)*period)<=3, `Native timestamp drift: ${JSON.stringify(ons)}`));
    for (const [frame,pitch] of ons.slice(0,-2))
        e2e.assert(trace.events.some(e=>!e.on && e.note===pitch &&
            Math.abs(e.frame-frame-period*0.7)<=3), 'Missing timestamped note off');
    e2e.assert(e2e.observe(250).peak>0.001, 'Sequence notes produced no audio');
}

// %% edit during playback without resetting the clock
repl.sequence.set({track:sequenceTest.track, steps:[[72],[76]], bpm:90, beats:0.25,
    gate:0.8, velocity:16000});
e2e.wait(1200);
{
    const ons = e2e.ons(e2e.trace().events, e2e.instrument()).filter(([,p])=>p>=72);
    const period = uapmd.sequencer.getSampleRate()/6;
    e2e.assert(ons.length >= 5, 'New sequence did not reach the audio thread');
    const oldSteps = (ons[0][0]-sequenceTest.origin)/sequenceTest.period;
    e2e.assert(Math.abs(oldSteps-Math.round(oldSteps))*sequenceTest.period<=3,
        'Edit did not wait for the next step boundary');
    ons.forEach(([frame],i)=>e2e.assert(Math.abs(frame-ons[0][0]-i*period)<=3,
        'New tempo has callback jitter'));
}

// %% error invalid edits preserve the running plan
repl.sequence.set({track:sequenceTest.track, steps:[[128]], bpm:90, beats:0.25,
    gate:0.8, velocity:16000});

// %% invalid edit did not interrupt playback
sequenceTest.before = e2e.trace().events.length;
e2e.wait(500);
e2e.assert(e2e.ons(e2e.trace().events.slice(sequenceTest.before)).length>=2,
    'Invalid edit discarded the active pattern');

// %% clear while a note is held
e2e.waitFor(()=>repl.test.notes().length>0,500);
repl.sequence.clear();
e2e.wait(250);
e2e.assert(repl.test.notes().length===0, 'Clear left hanging notes');
sequenceTest.before = e2e.trace().events.length;
e2e.wait(400);
e2e.assert(e2e.ons(e2e.trace().events.slice(sequenceTest.before)).length===0,
    'Clear left the native source running');

// %% arm a held chord
repl.sequence.set({track:sequenceTest.track, steps:[[50,57]], bpm:120, beats:2,
    gate:0.9, velocity:16000});
e2e.assert(e2e.waitFor(()=>repl.test.notes().length===2,500), 'Could not hold the test chord');

// %% stop with a held chord
e2e.stopped();

// %% restart retains the pattern and its first chord
repl.test.loopTrace(true);
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();
e2e.wait(250);
e2e.assert(JSON.stringify(e2e.ons(e2e.trace().events).map(([,p])=>p))==='[50,57]',
    'Restart lost or duplicated the initial chord');

// %% standard loop playback replaces the native source
{
    const loop = include('builtin/lib/loop.js');
    loop.playLoopParts({test:loop.prepareLoopPart(sequenceTest.track,'l4 b')},120);
    const before = e2e.trace().events.length;
    e2e.wait(1100);
    const ons = e2e.ons(e2e.trace().events.slice(before));
    e2e.assert(ons.length>=2 && ons.every(([,p])=>p===71),
        'The native source hid standard loop clips after switching');
}

// %% stop
e2e.stopped();
