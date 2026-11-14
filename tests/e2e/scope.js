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
