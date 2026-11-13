// The loop-playback patch on real plugin input, with updates while it plays:
// a fixed four-beat loop entered while ordinary transport runs, boundary and
// consecutive updates, a rounded fractional period with trailing rests,
// automatic lengthening and shortening, two parts on different render leads,
// and the tutorial reset. `loop.js` is the same case without the updates.
// Test-owned phrases: editable snippet contents are never expectations;
// pitches, onsets and periods come from the MML converter.

// %% helpers, native loop checks and a fresh plugin-input trace
globalThis.loopCase = (() => {
    const loopTrack = 0;
    const main = 'cegb';
    const loop = include('builtin/lib/loop.js');

    function play() {
        uapmd.audio.setEngineEnabled(true);
        if (!uapmd.timeline.getState().isPlaying) uapmd.sequencer.startPlayback();
    }

    // The test-owned entry: `mml` as part `main` at 120 BPM, then playing.
    function publish(mml) {
        const next = Object.assign({}, globalThis.loopParts || {});
        next.main = loop.prepareLoopPart(loopTrack, mml);
        loop.publishLoopParts(next, 120);
        play();
    }

    // Each track with its nodes and clips, as the timeline view reads them.
    const tracks = () => uapmd.sequencer.getTrackInfos().map((track, trackIndex) => ({
        trackIndex, nodes: track.nodes, clips: e2e.clips(trackIndex)
    }));

    return {loopTrack, main, loop, play, publish, tracks, sampleRate: uapmd.sequencer.getSampleRate()};
})();
repl.test.loopChecks();
loopCase.before = loopCase.tracks();
repl.test.loopTrace(true);
loopCase.sampleRate;

// %% ordinary transport already running
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% first loop publication
loopCase.publish(loopCase.main);

// %% fixed four-beat loop
loopCase.fixed = (() => {
    const {peak} = e2e.observe(6500);
    e2e.assert(peak > 0.001, 'Loop produced no output signal');
    const notes = e2e.ons(e2e.trace().events);
    e2e.assert(notes.length >= 12, `Too few loop notes: ${JSON.stringify(notes)}`);
    const origin = notes[0][0];
    const main = e2e.phrase(loopCase.main);
    notes.forEach(([frame, note], i) => e2e.assert(
        note === main.pitch(i) && Math.abs(frame - origin - main.at(i, main.period)) <= 1,
        `Loop timing/sequence mismatch at ${i}: ${JSON.stringify(notes)}`));
    e2e.assert(e2e.same(loopCase.tracks(), loopCase.before), 'Loop changed existing clips or plugin chain');
    return {origin, count: notes.length, peak};
})();

// %% boundary update
loopCase.old = e2e.ons(e2e.trace().events).length;
loopCase.publish('dfac');

// %% error - a missing loop MIDI file leaves the submitted plan untouched
repl.timeline.publishLoopClips([{trackIndex: 0, filepath: 'X:/missing-loop.mid'}]);

// %% the update lands on the next common boundary
(() => {
    e2e.wait(4400);
    const main = e2e.phrase(loopCase.main), next = e2e.phrase('dfac');
    const notes = e2e.ons(e2e.trace().events);
    const first = notes.findIndex(([, note]) => note === next.pitch(0));
    e2e.assert(first >= 0, 'New loop never reached plugin');
    const cycle = main.pitches.length;
    e2e.assert(first === Math.ceil(loopCase.old / cycle) * cycle, `Mid-cycle loop update: ${JSON.stringify(notes)}`);
    const boundary = (notes[first][0] - loopCase.fixed.origin) / main.period;
    e2e.assert(Math.abs(boundary - Math.round(boundary)) <= 1 / loopCase.sampleRate, 'Update was not at common boundary');
    notes.slice(first).forEach(([, note], i) => e2e.assert(note === next.pitch(i),
        `Old/new cycle mixed: ${JSON.stringify(notes)}`));
    return {first, boundary};
})();

// %% a first quick update
loopCase.publish('eeee');

// %% a second quick update
loopCase.publish('gggg');

// %% the latest fully prepared update before a boundary wins
(() => {
    e2e.wait(4300);
    const settled = e2e.phrase('gggg').pitch(0);
    const latest = e2e.ons(e2e.trace().events);
    e2e.assert(latest.slice(-4).every(([, note]) => note === settled),
        `Consecutive updates did not settle: ${JSON.stringify(latest)}`);
    return latest.slice(-4);
})();

// %% an update just before the user stops
loopCase.publish('aaaa');

// %% stop - with that update reserved
e2e.stopped();
e2e.wait(250);
e2e.assert(uapmd.timeline.getState().isPlaying === false, 'Stale update restarted transport');

// %% stop - the fixed loop
e2e.stopped();
repl.test.loopTrace(true);

// %% a rounded sample period with trailing rests, not a block multiple
{
    const next = Object.assign({}, globalThis.loopParts || {});
    next.main = loopCase.loop.prepareLoopPart(loopCase.loopTrack, 'l16 crrr');
    repl.timeline.publishLoopClips(Object.keys(next).map(k => next[k]), 1.234567, 120);
    globalThis.loopParts = next;
    loopCase.play();
}

// %% the fractional period repeats without drift
(() => {
    e2e.wait(3000);
    const single = e2e.phrase('l16 crrr').pitch(0);
    const period = Math.round(1.234567 * loopCase.sampleRate / 2);
    const notes = e2e.ons(e2e.trace().events);
    e2e.assert(notes.length >= 4 && notes.every(([, note]) => note === single)
        && notes.every(([frame], i) => i === 0 || frame - notes[i - 1][0] === period),
        `Fractional loop drift: period ${period}, ${JSON.stringify(notes)}`);
    return {period, count: notes.length};
})();

// %% stop - the fractional loop
e2e.stopped();

// %% automatic lengths, playing
loopCase.publish(loopCase.main);

// %% phrase lengths from the reference converter
for (const [mml, beats] of [['c4 r4', 2], ["'c4e4g4'", 1], ['c4. r8', 2], ['c4 r4;e2 r4', 3], ['{"x":1}c4 r4', 2]]) {
    e2e.assert(loopCase.loop.loopPhraseBeats(mml) === beats, `Phrase end: ${mml}`);
}

// %% stop - before the automatic phrases
e2e.stopped();
repl.test.loopTrace(true);

// %% automatic initial phrase
loopCase.publish(loopCase.main);
e2e.wait(900);

// %% automatic longer phrase lengthens the period at a boundary
loopCase.auto = (() => {
    loopCase.publish('dfacga');
    const {peak} = e2e.observe(7000);
    const old = e2e.phrase(loopCase.main), long = e2e.phrase('dfacga');
    const extended = e2e.ons(e2e.trace().events);
    const transport = uapmd.timeline.getState();
    e2e.assert(transport.loopEndSamples === long.period
        && transport.playheadSamples >= 0 && transport.playheadSamples < long.period,
        `Extended transport disagrees: ${JSON.stringify(transport)}`);
    const first = extended.findIndex(([, note]) => note === long.pitch(0));
    e2e.assert(first >= 0, 'No extension');
    e2e.assert(first > 0 && first % old.pitches.length === 0,
        `Extension interrupted old cycle: ${JSON.stringify(extended)}`);
    const origin = extended[0][0];
    const change = Math.floor(first / old.pitches.length) * old.period;
    extended.forEach(([frame, note], i) => {
        const [pitch, at] = i < first
            ? [old.pitch(i), old.at(i, old.period)]
            : [long.pitch(i - first), change + long.at(i - first, long.period)];
        e2e.assert(note === pitch && Math.abs(frame - origin - at) <= 1,
            `Extended tail, timing or origin mismatch: ${JSON.stringify(extended)}`);
    });
    e2e.assert(extended.length >= first + 2 * long.pitches.length && peak > 0.001,
        'Insufficient longer-phrase delivery or master signal');
    return {first, firstFrame: extended[first][0], extensionPeak: peak};
})();

// %% automatic shorter phrase keeps its trailing rests
(() => {
    loopCase.publish('err');
    const {peak} = e2e.observe(5500);
    const long = e2e.phrase('dfacga'), short = e2e.phrase('err');
    const contracted = e2e.ons(e2e.trace().events);
    e2e.assert(uapmd.timeline.getState().loopEndSamples === short.period, 'Shortened transport disagrees');
    const index = contracted.findIndex(([, note], i) => i >= loopCase.auto.first && note === short.pitch(0));
    e2e.assert(index >= 0, 'No contraction');
    const boundary = contracted[index][0];
    e2e.assert((index - loopCase.auto.first) % long.pitches.length === 0
        && (boundary - loopCase.auto.firstFrame) % long.period === 0,
        `Shortening broke the longer-phrase boundary: ${JSON.stringify(contracted)}`);
    const tail = contracted.slice(index);
    e2e.assert(tail.length >= 2 && tail.every(([, note]) => note === short.pitch(0))
        && tail.every(([frame], i) => i === 0 || frame - tail[i - 1][0] === short.period) && peak > 0.001,
        `Trailing rests lost: ${JSON.stringify(tail)}`);
    loopCase.auto.short = index;
    loopCase.auto.saved = JSON.stringify(globalThis.loopParts);
    return {index, period: short.period, shortPeak: peak};
})();

// %% error - an empty phrase is rejected
loopCase.publish('');

// %% error - a phrase with its own tempo is rejected
loopCase.publish(`t90 ${loopCase.main}`);

// %% invalid input keeps the map, notes and period
(() => {
    e2e.assert(JSON.stringify(globalThis.loopParts) === loopCase.auto.saved, 'Invalid input replaced script map');
    e2e.wait(2000);
    const short = e2e.phrase('err');
    const delivered = e2e.trace().events;
    const after = e2e.ons(delivered).slice(loopCase.auto.short);
    e2e.assert(after.every(([, note]) => note === short.pitch(0))
        && after.every(([frame], i) => i === 0 || frame - after[i - 1][0] === short.period),
        'Invalid input damaged active plan');
    const held = new Set();
    for (const event of delivered) {
        if (event.on) {
            e2e.assert(!held.has(event.note), `Double note-on: ${JSON.stringify(event)}`);
            held.add(event.note);
        } else {
            held.delete(event.note);
        }
    }
    return after.length;
})();

// %% an update just before the user stops the automatic phrases
loopCase.publish('aaaaaa');

// %% stop - the automatic phrases
e2e.stopped();

// %% a second Surge XT for a second part
loopCase.second = uapmd.instancing.create('CLAP', 'org.surge-synth-team.surge-xt');

// %% an effect whose state must be preserved
loopCase.effect = uapmd.instancing.create('CLAP', 'org.surge-synth-team.surge-xt-fx', e2e.trackId(0));

// %% an ordinary clip and the states the loop must not change
(() => {
    loopCase.instrument = e2e.instrument();
    const {second, effect, instrument} = loopCase;
    e2e.assert(second >= 0 && second !== instrument && effect >= 0, 'Multi-part fixture instances were not created');
    uapmd.timeline.createEmptyMidiClip(e2e.trackId(0), 12345, 480, 120);
    loopCase.states = [repl.test.pluginState(instrument), repl.test.pluginState(effect)];
    loopCase.topology = loopCase.tracks();
    repl.test.loopTrace(true);
    return {second, effect, instrument};
})();

// %% the main part, published without playing
{
    const next = Object.assign({}, globalThis.loopParts || {});
    next.main = loopCase.loop.prepareLoopPart(loopCase.loopTrack, loopCase.main);
    loopCase.loop.publishLoopParts(next, 120);
}

// %% the second part on its own track, then playing
{
    loopCase.secondTrack = include('builtin/lib/plugins.js').trackFor(loopCase.second);
    const next = Object.assign({}, globalThis.loopParts);
    next.second = loopCase.loop.prepareLoopPart(loopCase.secondTrack, 'dddd');
    loopCase.loop.publishLoopParts(next, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}

// %% two common-phase parts
loopCase.initialPeak = e2e.observe(2700).peak;

// %% both parts prepared before one publication
{
    const mainNext = getGeneratedMidiPath(), secondNext = getGeneratedMidiPath();
    mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('ffff'), mainNext);
    mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('aaaa'), secondNext);
    repl.timeline.publishLoopClips([
        {trackIndex: loopParts.main.trackIndex, filepath: mainNext},
        {trackIndex: loopCase.secondTrack, filepath: secondNext}
    ]);
}

// %% the update reaches both parts at one common time
(() => {
    e2e.wait(4200);
    const {instrument, second} = loopCase;
    const base = e2e.phrase(loopCase.main);
    const [updatedA, updatedB] = [e2e.phrase('ffff').pitch(0), e2e.phrase('aaaa').pitch(0)];
    const {events, timing} = e2e.trace();
    const lead = instance => {
        const track = timing.find(t => t.instances.includes(instance));
        e2e.assert(track, 'Missing track render lead');
        return track.lead;
    };
    const [aLead, bLead] = [lead(instrument), lead(second)];
    const a = e2e.ons(events, instrument), b = e2e.ons(events, second);
    const ia = a.findIndex(([, note]) => note === updatedA);
    const ib = b.findIndex(([, note]) => note === updatedB);
    e2e.assert(ia >= 0, 'Main part update missing');
    e2e.assert(ib >= 0, 'Second part update missing');
    e2e.assert(a.length >= 12 && b.length >= 12 && ia > 0 && ia % base.pitches.length === 0 && ib === ia
        && aLead !== bLead && a[ia][0] + aLead === b[ib][0] + bLead
        && a.every(([frame], i) => i >= b.length || frame + aLead === b[i][0] + bLead)
        && a.slice(ia).every(([, note]) => note === updatedA)
        && b.slice(ib).every(([, note]) => note === updatedB)
        && Math.abs(a[ia][0] - a[0][0] - Math.floor(ia / base.pitches.length) * base.period) <= 1,
        `Multi-part phase/update mismatch: ${JSON.stringify(a)}, ${JSON.stringify(b)}`);
    loopCase.leads = [aLead, bLead];
    return {aLead, bLead, update: ia};
})();

// %% the longest part sets the common period of both tracks
(() => {
    const next = Object.assign({}, globalThis.loopParts);
    next.main = loopCase.loop.prepareLoopPart(loopCase.loopTrack, 'f+f+rrrr');
    next.second = loopCase.loop.prepareLoopPart(loopCase.secondTrack, 'brr');
    loopCase.loop.publishLoopParts(next, 120);
    const {peak} = e2e.observe(7200);
    const [pa, pb] = [e2e.phrase('f+f+rrrr'), e2e.phrase('brr')];
    const cycle = Math.max(pa.period, pb.period);
    const [aLead, bLead] = loopCase.leads;
    const {events} = e2e.trace();
    const a = e2e.ons(events, loopCase.instrument), b = e2e.ons(events, loopCase.second);
    const ia = a.findIndex(([, note]) => note === pa.pitch(0));
    const ib = b.findIndex(([, note]) => note === pb.pitch(0));
    e2e.assert(ia >= 0, 'Main extension missing');
    e2e.assert(ib >= 0, 'Second extension missing');
    const common = a[ia][0] + aLead;
    e2e.assert(aLead !== bLead && common === b[ib][0] + bLead && b.length >= ib + 2 && a.length >= ia + 4,
        `Dynamic lead alignment missing: ${JSON.stringify(a)} ${JSON.stringify(b)}`);
    a.slice(ia).forEach(([frame, note], i) => e2e.assert(
        note === pa.pitch(i) && frame + aLead === common + pa.at(i, cycle),
        `Main long period mismatch: ${JSON.stringify(a)}`));
    b.slice(ib).forEach(([frame, note], i) => e2e.assert(
        note === pb.pitch(i) && frame + bLead === common + pb.at(i, cycle),
        `Short part repeated independently: ${JSON.stringify(b)}`));
    loopCase.extended = {ia, common, cycle, peak};
    return {common, cycle, peak};
})();

// %% shortening the longest part leaves the second part longest
(() => {
    const next = Object.assign({}, globalThis.loopParts);
    next.main = loopCase.loop.prepareLoopPart(loopCase.loopTrack, 'c');
    loopCase.loop.publishLoopParts(next, 120);
    const {peak} = e2e.observe(6000);
    const [pb, pc] = [e2e.phrase('brr'), e2e.phrase('c')];
    const [aLead, bLead] = loopCase.leads;
    const {ia, common, cycle} = loopCase.extended;
    const {events} = e2e.trace();
    const a = e2e.ons(events, loopCase.instrument), b = e2e.ons(events, loopCase.second);
    const short = a.findIndex(([, note], i) => i >= ia && note === pc.pitch(0));
    e2e.assert(short >= 0, 'Main shortening missing');
    const boundary = a[short][0] + aLead;
    const bShort = b.findIndex(([frame]) => frame + bLead === boundary);
    e2e.assert(bShort >= 0, 'Second phase not shared on shortening');
    const period = Math.max(pc.period, pb.period);
    const steady = notes => notes.every(([frame], i) => i === 0 || frame - notes[i - 1][0] === period);
    e2e.assert((boundary - common) % cycle === 0 && a.length >= short + 2 && b.length >= bShort + 2
        && a.slice(short).every(([, note]) => note === pc.pitch(0))
        && steady(a.slice(short)) && steady(b.slice(bShort))
        && loopCase.extended.peak > 0.001 && peak > 0.001,
        `Common period shortening mismatch: ${JSON.stringify(a)} ${JSON.stringify(b)}`);
    return {boundary, period, peak};
})();

// %% stop - the parts; clips, instrument and effect are unchanged
(() => {
    e2e.stopped();
    const states = [repl.test.pluginState(loopCase.instrument), repl.test.pluginState(loopCase.effect)];
    e2e.assert(e2e.same(loopCase.tracks(), loopCase.topology) && e2e.same(states, loopCase.states),
        'Loop update changed existing clips, instrument or effect state');
    const status = repl.timeline.loopStatus();
    e2e.assert(status.dropped === 0 && loopCase.initialPeak > 0.001,
        `Multi-part delivery overflow or missing signal: ${JSON.stringify(status)}`);
    return {status, stateBytes: states.map(s => s.length / 2)};
})();

// %% loop before the tutorial reset
repl.test.loopTrace(true);
loopCase.publish(loopCase.main);
e2e.wait(1200);
e2e.assert(e2e.ons(e2e.trace().events).length > 0, 'Loop did not play before reset');

// %% tutorial reset
include('builtin/lib/tutorial.js').reset();

// %% stop - the input trace restarts only with audio stopped
e2e.stopped();
repl.test.loopTrace(true);

// %% a later phrase, longer than the old loop, at position 0
{
    for (const track of uapmd.sequencer.getTrackInfos())
        for (const node of track.nodes.filter(node => node.instanceId >= 0))
            uapmd.sequencer.setPluginBypassed(node.instanceId, false);
    const laterPath = getGeneratedMidiPath();
    mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('l2 cde'), laterPath);
    const clipTrack = 0;
    const clip = uapmd.timeline.addMidiClip(e2e.trackId(clipTrack), 0, laterPath);
    if (!clip.success) throw new Error(clip.error);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}

// %% the later phrase plays through without folding
(() => {
    const later = e2e.phrase('l2 cde');
    const span = later.at(later.pitches.length - 1, later.period) / loopCase.sampleRate;
    e2e.wait(span * 1000 + 800);
    const state = uapmd.timeline.getState();
    // Folding never reaches the later pitches; repeats of one pitch before the next are ignored.
    const notes = e2e.ons(e2e.trace().events, e2e.instrument())
        .filter(([, note], i, all) => i === 0 || note !== all[i - 1][1]);
    e2e.assert(state.loopEnabled === false && state.loopEndSamples === 0 && globalThis.loopParts === undefined,
        `Loop survived reset: ${JSON.stringify(state)}, loopParts cleared: ${globalThis.loopParts === undefined}`);
    const pitches = notes.map(([, note]) => note);
    e2e.assert(pitches[0] === later.pitches[0], `First note missing after reset: ${JSON.stringify(notes)}`);
    e2e.assert(e2e.same(pitches, later.pitches), `Phrase folded after reset: ${JSON.stringify(notes)}`);
    return notes;
})();

// %% stop - after the reset
e2e.stopped();
