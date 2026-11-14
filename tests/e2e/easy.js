// `builtin/lib/easy.js`: the i-th input goes to track i, only those tracks sound,
// reset empties every track, and
// `builtin/lib/effect.js` places presets as cat-smf-editor does.
// Test-owned phrases, plugins and presets: editable snippet contents are never expectations.

// %% define the easy functions
{
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', bpm:120});
    globalThis.easyTest = {
        easy,
        clipCounts: () => uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackId).length),
        rejects: (f, what) => {
            let threw = false;
            try { f(); } catch (e) { threw = true; }
            e2e.assert(threw, `${what} did not throw`);
        },
    };
    e2e.assert(uapmd.sequencer.getTrackInfos().length >= 3, 'The case needs three tracks');
}

// %% one input plays on track 0 alone
{
    const {easy, clipCounts} = easyTest;
    const played = easy.play('c');
    e2e.assert(e2e.same(played, [0]), `play went to ${JSON.stringify(played)}`);
    e2e.assert(e2e.same(clipCounts().slice(0, 3), [1, 0, 0]), `clips: ${JSON.stringify(clipCounts())}`);
}

// %% two inputs play on tracks 0 and 1
{
    const {easy, clipCounts} = easyTest;
    const played = easy.play('c', 'e');
    e2e.assert(e2e.same(played, [0, 1]), `play went to ${JSON.stringify(played)}`);
    e2e.assert(include('builtin/lib/plugins.js').instrumentOn(1) >= 0, 'Track 1 got no instrument');
    e2e.assert(e2e.same(clipCounts().slice(0, 3), [1, 1, 0]), `clips: ${JSON.stringify(clipCounts())}`);
    e2e.assert(uapmd.timeline.getState().isPlaying, 'play did not start playback');
}

// %% fewer inputs leave the other tracks silent
{
    const {easy, clipCounts} = easyTest;
    const played = easy.play('g');
    e2e.assert(e2e.same(played, [0]), `play went to ${JSON.stringify(played)}`);
    e2e.assert(e2e.same(clipCounts().slice(0, 3), [1, 0, 0]), `clips: ${JSON.stringify(clipCounts())}`);
}

// %% playLoop loops from track 0
{
    const {easy} = easyTest;
    const looped = easy.playLoop('c', 'e');
    e2e.assert(e2e.same(looped, [0, 1]), `playLoop went to ${JSON.stringify(looped)}`);
    const tracks = Object.values(globalThis.loopParts).map(p => p.trackIndex).sort();
    e2e.assert(e2e.same(tracks, [0, 1]), `loop parts: ${JSON.stringify(tracks)}`);
    e2e.wait(500);
    const fewer = easy.playLoop('g');
    const left = Object.values(globalThis.loopParts).map(p => p.trackIndex);
    e2e.assert(e2e.same(fewer, [0]) && e2e.same(left, [0]), `a shorter playLoop left ${JSON.stringify(left)}`);
}

// %% wrong calls throw
{
    const {easy, rejects} = easyTest;
    rejects(() => easy.play(), 'play without input');
    rejects(() => easy.changePatch(99), 'changePatch on a missing track');
    rejects(() => easy.changeEffect(), 'changeEffect without a track');
    const trackCount = uapmd.sequencer.getTrackInfos().length;
    rejects(() => easy.play(...Array(trackCount + 1).fill('c')), 'play past the last track');
    e2e.assert(e2e.same(easy.play('c'), [0]), 'play after a failed call missed track 0');
}

// %% effect presets: same plugin reloads, another replaces the last effect
{
    const effectTrack = 2;
    const plugins = include('builtin/lib/plugins.js');
    const effect = include('builtin/lib/effect.js');
    const surge = preset => effect.use({format:'CLAP', plugin:'Surge XT Effects', preset, value:preset, track:effectTrack});
    const first = surge('Airwindows/Ambience/Chamber.srgfx').instanceId;
    e2e.assert(e2e.same(plugins.effectsOn(effectTrack), [first]), 'The first preset did not add one effect');
    const second = surge('Airwindows/Ambience/Galactic.srgfx').instanceId;
    e2e.assert(second === first, 'A preset of the same plugin did not reuse its effect');
    const hall = effect.use({format:'CLAP', plugin:'Dragonfly Hall Reverb', preset:'', value:'Bright Room', track:effectTrack}).instanceId;
    e2e.wait(300);
    e2e.assert(e2e.same(plugins.effectsOn(effectTrack), [hall]), 'Another plugin did not replace the last effect');
    const instrument = plugins.instrumentOn(effectTrack);
    const connections = plugins.connectionsOf(effectTrack).filter(c => c.busType === 'audio')
        .map(c => [c.source.type === 'plugin' ? c.source.instanceId : c.source.type,
            c.target.type === 'plugin' ? c.target.instanceId : c.target.type]);
    e2e.assert(connections.some(([s, t]) => s === instrument && t === hall)
        && connections.some(([s, t]) => s === hall && t === 'graphOutput'),
        `The replaced effect is not between the instrument and the output: ${JSON.stringify(connections)}`);
}

// %% reset removes playback, clips and plugins
{
    const {easy, clipCounts} = easyTest;
    easy.playLoop('c');
    easy.reset();
    const left = uapmd.sequencer.getTrackInfos().flatMap(t => t.nodes.filter(n => n.instanceId >= 0).map(n => n.instanceId));
    e2e.assert(left.length === 0, `reset left plugins ${JSON.stringify(left)}`);
    e2e.assert(clipCounts().every(c => c === 0), `reset left clips: ${JSON.stringify(clipCounts())}`);
    e2e.assert(!uapmd.timeline.getState().isPlaying && globalThis.loopParts === undefined, 'reset left playback');
}

// %% after reset, play gives track 0 the plugin with the patch
{
    const {clipCounts} = easyTest;
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Basses/Attacky.fxp', bpm:120});
    e2e.assert(e2e.same(easy.play('c'), [0]), 'play after reset missed track 0');
    e2e.assert(include('builtin/lib/plugins.js').instrumentOn(0) >= 0, 'play after reset put no instrument');
    e2e.assert(e2e.same(clipCounts().slice(0, 3), [1, 0, 0]), `clips: ${JSON.stringify(clipCounts())}`);
}

// %% after reset, repeated plays keep one connected instrument per track
// A changed patch lays the track's connections, which outlive the reset.
{
    const easy = include('builtin/lib/easy.js').create({format:'CLAP', plugin:'Surge XT', bpm:120});
    const plugins = include('builtin/lib/plugins.js');
    for (const track of [0, 1]) include('builtin/lib/patch.js').use({track, format:'CLAP', plugin:'Dexed'});
    easy.reset();
    for (let i = 0; i < 3; i++) easy.playLoop('c', 'e');
    easy.play('c', 'e');
    for (const track of [0, 1]) {
        const nodes = uapmd.sequencer.getTrackInfos()[track].nodes.filter(n => n.instanceId >= 0);
        e2e.assert(nodes.length === 1 && plugins.instrumentOn(track) === nodes[0].instanceId,
            `track ${track} holds ${JSON.stringify(nodes.map(n => n.instanceId))}, instrument ${plugins.instrumentOn(track)}`);
    }
}
