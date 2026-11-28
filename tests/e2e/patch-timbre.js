// Two patches of one plugin, restored by display path with `patch.use`, make a
// different output for the same note; the first patch again makes nearly the
// same output as before, which bounds the difference noise alone can make.
// Two presets of one effect, restored with `effect.use`, do the same on the
// output of a track's instrument.
// Test-owned script: editable snippet contents are never expectations.

// %% helpers
globalThis.patchTimbre = (() => {
    const {assert, wait} = e2e;
    const note = 60;

    // The master output's rms every 25 ms while the note is held for `held` ms
    // and for `released` ms after, once the output went silent from any earlier note.
    function profile(instanceId, held = 800, released = 0) {
        const quiet = e2e.waitFor(() => repl.test.observe().peak <= 1e-5, 5000);
        assert(quiet, `Output did not go silent before the note: ${JSON.stringify(repl.test.observe())}`);
        uapmd.sequencer.sendNoteOn(instanceId, note);
        const rms = [];
        const record = ms => {
            const end = Date.now() + ms;
            while (Date.now() < end) {
                const value = repl.test.observe();
                assert(value.finite, `Invalid observation: ${JSON.stringify(value)}`);
                rms.push(value.rms);
                wait(25);
            }
        };
        record(held);
        uapmd.sequencer.sendNoteOff(instanceId, note);
        record(released);
        assert(Math.max(...rms) > 1e-3, `No note signal: ${JSON.stringify(rms)}`);
        return rms;
    }

    // 0 for profiles holding the same levels, 1 when one of them is silent.
    // Levels are compared in order of size, as samples do not fall on the same
    // instants of two notes.
    function distance(a, b) {
        const n = Math.min(a.length, b.length);
        a = [...a].sort((x, y) => x - y);
        b = [...b].sort((x, y) => x - y);
        let apart = 0, scale = 0;
        for (let i = 0; i < n; i++) {
            apart += Math.abs(a[i] - b[i]);
            scale += Math.max(a[i], b[i]);
        }
        return apart / scale;
    }

    // Loaded states apply on some plugins' audio thread, so this waits `settle`
    // ms with the engine running before the note.
    function sound(track, pluginId, patch, settle = 500) {
        repl.processEvents();
        const {instanceId} = include('builtin/lib/patch.js').use({track, format:'CLAP', pluginId, patch});
        wait(settle);
        return profile(instanceId);
    }

    function compare(name, sound, first, second) {
        uapmd.audio.setEngineEnabled(true);
        const a = sound(first);
        const b = sound(second);
        const again = sound(first);
        const result = {name, between:distance(a, b), again:distance(a, again)};
        assert(result.between > 0.2 && result.between > 3 * result.again,
            `Presets did not differ beyond noise: ${JSON.stringify(result)}`);
        return result;
    }

    function differ(track, pluginId, first, second, settle) {
        return compare(pluginId, patch => sound(track, pluginId, patch, settle), first, second);
    }

    // The track's instrument through the effect `plugin` (its name) with each
    // preset {preset, value}; the note's tail after release is part of the output.
    // Through a distortion the level of the instrument's attack varies from note
    // to note, which a long `held` dilutes; a reverb's presets differ in the tail,
    // which a long `released` brings out.
    function effectDiffer(track, plugin, first, second, held = 400, released = 1200) {
        const instrument = include('builtin/lib/plugins.js').instrumentOn(track);
        assert(instrument >= 0, `Track ${track} has no instrument`);
        return compare(plugin, ({preset, value}) => {
            repl.processEvents();
            include('builtin/lib/effect.js').use({format:'CLAP', plugin, preset, value, track});
            wait(500);
            return profile(instrument, held, released);
        }, first, second);
    }

    return {differ, effectDiffer};
})();

// %% Surge XT on the startup instrument
{
    const startTrack = 0;
    patchTimbre.differ(startTrack, 'org.surge-synth-team.surge-xt',
        'patches_factory/Basses/Attacky.fxp', 'patches_factory/Leads/Acidofil.fxp');
}

// %% Six Sines on a track without a plugin
// A plugin that replaces track 0's instrument leaves the master output silent;
// one on a new track sounds.
{
    const emptyTrack = 1;
    patchTimbre.differ(emptyTrack, 'org.baconpaul.six-sines', 'Bass/Bass 1.sxsnp', 'Leads/Airlock.sxsnp');
}

// %% sforzando on an instance `use` did not create
// As the browser leaves one: no state of a new sforzando is kept yet, so its
// states are built on the state of another new instance made for this.
{
    const job = uapmd.sequencer.addTrack();
    e2e.assert(job.state === 'completed', `addTrack did not add a track: ${JSON.stringify(job)}`);
    const newTrack = uapmd.sequencer.getTrackInfos().length - 1;
    const plugins = include('builtin/lib/plugins.js');
    const pluginId = 'com.Plogue Art et Technologie, Inc.sforzando';
    const instanceId = uapmd.instancing.create('CLAP', pluginId, include('builtin/lib/tracks.js').idOf(newTrack));
    e2e.assert(instanceId >= 0, 'sforzando was not created');
    plugins.joinAlone(newTrack, instanceId);
    const result = patchTimbre.differ(newTrack, pluginId,
        'Free Sounds/Programs/DSF/Huge Brass.sfz', 'Free Sounds/Programs/DSF/Clav 1.sfz');
    e2e.assert(include('builtin/lib/patch.js').use({track:newTrack, format:'CLAP', pluginId}).instanceId === instanceId,
        'patch.use replaced the instance');
    result;
}

// %% sforzando on a track without a plugin
// Its states are built on a kept template.
{
    const emptyTrack = 2;
    patchTimbre.differ(emptyTrack, 'com.Plogue Art et Technologie, Inc.sforzando',
        'Free Sounds/Programs/DSF/Clav 1.sfz', 'Free Sounds/Programs/DSF/Huge Brass.sfz');
}

// %% effect presets on the startup instrument
// Surge XT Effects takes states prepared ahead; Dragonfly Hall Reverb and
// TONE3000 build theirs on the state of the instance `use` creates.
{
    const instrumentTrack = 0;
    const file = preset => ({preset, value:preset});
    [
        patchTimbre.effectDiffer(instrumentTrack, 'Surge XT Effects',
            file('Distortion/Crunchy.srgfx'), file('Distortion/Overdrive.srgfx'), 1200),
        patchTimbre.effectDiffer(instrumentTrack, 'Dragonfly Hall Reverb',
            {preset:'', value:'Bright Room'}, {preset:'', value:'Great Hall'}, 400, 2400),
        patchTimbre.effectDiffer(instrumentTrack, 'TONE3000',
            {preset:'Factory/Ampeg Bass Drive.t3kpreset', value:'Factory/Ampeg Bass Drive'},
            {preset:'Factory/Matchless Crunch.t3kpreset', value:'Factory/Matchless Crunch'}),
    ];
}

// %% Floe on a new track
// Floe loads its preset into the instance with its own extension, after `use` returns;
// a first instance starts sounding about 0.6 s later.
{
    const job = uapmd.sequencer.addTrack();
    e2e.assert(job.state === 'completed', `addTrack did not add a track: ${JSON.stringify(job)}`);
    const newTrack = uapmd.sequencer.getTrackInfos().length - 1;
    patchTimbre.differ(newTrack, 'com.floe-audio.floe',
        'Celtic Harp Factory Presets/Realistic Celtic Harp.floe-preset',
        'Ocarina Factory Presets/Real Ocarina.floe-preset', 1500);
}

// %% Dexed on a new track
// Dexed takes a cartridge program as a voice SysEx on its audio thread, after
// `use` returns; program 0 left sounding for program 1 fails here.
{
    const job = uapmd.sequencer.addTrack();
    e2e.assert(job.state === 'completed', `addTrack did not add a track: ${JSON.stringify(job)}`);
    const newTrack = uapmd.sequencer.getTrackInfos().length - 1;
    patchTimbre.differ(newTrack, 'com.digital-suburban.dexed',
        'SynprezFM/SynprezFM_14.syx/00 THE  FIX', 'SynprezFM/SynprezFM_14.syx/01 Bill Bell');
}
