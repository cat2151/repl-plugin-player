// `scope`: the scope and the keyboard display follow the instrument on track 0,
// across a Stop and a replaced instrument. The checks are Rust
// (`src/scope_e2e.rs`); it runs these cells by title.

// %% init
include('builtin/init.js')

// %% instrument
include('builtin/lib/plugins.js').instrumentOn(0)

// %% scope target
repl.scope.target()

// %% plugins on track 0
uapmd.sequencer.getTrackInfos()[0].nodes.filter(n => n.instanceId >= 0).length

// %% note 60
{
    const id = include('builtin/lib/plugins.js').instrumentOn(0);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.sendNoteOn(id, 60);
}

// %% note 72 after 60
{
    const id = include('builtin/lib/plugins.js').instrumentOn(0);
    uapmd.sequencer.sendNoteOff(id, 60);
    uapmd.sequencer.sendNoteOn(id, 72);
}

// %% note 67 with the engine on
{
    const id = include('builtin/lib/plugins.js').instrumentOn(0);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.sendNoteOn(id, 67);
}

// %% release 67
uapmd.sequencer.sendNoteOff(include('builtin/lib/plugins.js').instrumentOn(0), 67);

// %% replace the instrument
// A test-owned plugin other than the startup one, set on track 0 as cells do.
{
    const track = 0;
    const plugins = include('builtin/lib/plugins.js');
    const now = uapmd.sequencer.getTrackInfos()[track].nodes
        .find(n => n.instanceId === plugins.instrumentOn(track));
    const plugin = now.pluginId === 'com.digital-suburban.dexed' ? 'Surge XT' : 'Dexed';
    include('builtin/lib/patch.js').use({format:'CLAP', plugin, track});
}

// %% note 64
uapmd.sequencer.sendNoteOn(include('builtin/lib/plugins.js').instrumentOn(0), 64);

// %% easy reset and play
// After the easy layer's reset disposed every instrument, the one play creates goes on the scope.
{
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', bpm:120});
    easy.reset();
    easy.play("l1c");
}

// %% reload project
{
    const melodyTrack = 0;
    const otherTrack = 1;
    const plugins = include('builtin/lib/plugins.js');
    include('builtin/lib/patch.js').use({track:melodyTrack, format:'CLAP', plugin:'Surge XT',
        patch:'patches_factory/Templates/Init Saw.fxp'});
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'l1g', 120);
    const job = uapmd.sequencer.addTrack();
    if (job.state !== 'completed') throw new Error('scope test track creation failed');
    const id = plugins.ensureInstrument(otherTrack, {format:'CLAP', plugin:'Surge XT'});
    repl.scope.setTarget(id);
    const track = uapmd.sequencer.getTrackInfos()[otherTrack];
    const node = track.nodes.find(node => node.instanceId === id);
    repl.project.saveArchive(scopeProjectPath);
    for (let attempt = 0; attempt < 2; ++attempt) {
        const before = repl.scope.target();
        repl.project.load(scopeProjectPath);
        const restored = uapmd.sequencer.getTrackInfos().find(t => t.trackId === track.trackId);
        const selected = restored.nodes.find(n => n.nodeId === node.nodeId);
        if (repl.scope.target() !== selected.instanceId || repl.scope.target() === before)
            throw new Error('scope did not follow the second track across load');
    }
    const beforeFailure = repl.scope.target();
    try { repl.project.load(scopeProjectPath + '.missing'); } catch (_) {}
    if (repl.scope.target() !== beforeFailure) throw new Error('failed load changed scope');
    // A target absent from the incoming document falls back to its first instrument.
    repl.scope.setTarget(999999);
    repl.project.load(scopeProjectPath);
    if (repl.scope.target() !== plugins.instrumentOn(melodyTrack))
        throw new Error('scope did not default to the first instrument');
    for (const t of uapmd.sequencer.getTrackInfos())
        for (const n of t.nodes.filter(n => n.instanceId >= 0)) uapmd.instance(n.instanceId).dispose();
    repl.processEvents();
    repl.project.saveArchive(scopeProjectPath + '.empty.uapmdz');
    repl.project.load(scopeProjectPath + '.empty.uapmdz');
    if (repl.scope.target() !== -1) throw new Error('empty project retained a scope target');
    repl.project.load(scopeProjectPath);
    if (repl.scope.target() !== plugins.instrumentOn(melodyTrack))
        throw new Error('load without an outgoing target did not select an instrument');
}

// %% play loaded project
repl.timeline.setLoop(0, Math.round(uapmd.sequencer.getSampleRate() * 2));
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% freeze loaded project
{
    repl.timeline.clearLoop();
    const melodyTrack = 0;
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'l1a', 120);
    const track = uapmd.sequencer.getTrackInfos()[melodyTrack];
    uapmd.sequencer.setTrackFreezePolicy(track.trackId, 'on');
}

// %% frozen ready
(() => {
    const melodyTrack = 0;
    const track = uapmd.sequencer.getTrackInfos()[melodyTrack];
    const state = uapmd.sequencer.getTrackFreezeState(track.trackId);
    if (state.runtimeState === 'error') throw new Error(JSON.stringify(state));
    return state.runtimeState === 'frozen';
})();
