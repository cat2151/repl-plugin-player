// `browser-change`: the browsers that change track 0 at once, against a
// stand-in player. The checks and the stand-in are Rust
// (`src/browser_change_e2e.rs`); it runs these cells by title.
// `written effect cell` is the text the effect browser must write.
// Test-owned script: editable snippet contents are never expectations.

// %% loop
{
    const loopTrack = 0;
    const {prepareLoopPart, playLoopParts} = include('builtin/lib/loop.js');
    playLoopParts({main:prepareLoopPart(loopTrack, 'cegb')}, 120);
}

// %% change patch
{
    const track = 0;
    include('builtin/lib/patch.js').change({track});
}

// %% easy changePatch
{
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', bpm:120});
    easy.changePatch(0);
}

// %% change effect
{
    const track = 0;
    include('builtin/lib/effect.js').change({track});
}

// %% easy changeEffect
{
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', bpm:120});
    easy.changeEffect(0);
}

// %% track 0
// The plugin ids of track 0's instrument and effects.
(() => {
    const track = 0;
    const plugins = include('builtin/lib/plugins.js');
    const nodes = uapmd.sequencer.getTrackInfos()[track].nodes;
    const pluginOf = id => nodes.find(n => n.instanceId === id).pluginId;
    return {instrument:pluginOf(plugins.instrumentOn(track)), effects:plugins.effectsOn(track).map(pluginOf)};
})()

// %% written effect cell
{
    const track = 0;
    include('builtin/lib/effect.js').use({format:'CLAP', plugin:'Surge XT Effects', preset:'Airwindows/Ambience/Chamber.srgfx', value:'Airwindows/Ambience/Chamber.srgfx', track});
}
