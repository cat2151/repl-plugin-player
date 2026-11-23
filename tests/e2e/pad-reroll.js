// %% reroll patches and switch instrument pads between a note and a chord
{
    const id = 'E2E pad reroll';
    const format = 'CLAP';
    const plugin = 'Surge XT';
    const pluginId = 'org.surge-synth-team.surge-xt';
    const patches = ['patches_factory/Templates/Init Saw.fxp', 'patches_factory/Basses/Attacky.fxp'];
    const catalog = repl.catalogInstruments;
    const patchState = repl.patchState;
    const poll = repl.ui.surface.poll;
    const sequencer = uapmd.sequencer;
    const noteOn = sequencer.sendNoteOn;
    const noteOff = sequencer.sendNoteOff;
    const pending = [];
    const loads = [];
    const instanceIds = () => [...uapmd.sequencer.getInstanceIds()].sort((a, b) => a - b);
    function send(...events) {
        pending.push(...events.map(([kind, region]) => ({kind, region, x:0.5, y:0.5})));
        e2e.assert(e2e.waitFor(() => pending.length === 0, 3000), 'The pad task did not poll');
    }
    repl.catalogInstruments = () => [{format, plugin, pluginId, patches}];
    repl.patchState = options => {
        const result = patchState(options);
        loads.push(options);
        return result;
    };
    repl.ui.surface.poll = surfaceId => {
        const frame = poll(surfaceId);
        if (surfaceId === id) frame.events.push(...pending.splice(0));
        return frame;
    };
    try {
        include('builtin/lib/performance-pads.js').open({
            id, width:960, height:512,
            xy:{notes:[60], controller:1, color:[45, 56, 76]},
            instrumentNote:60, instrumentChord:'bIIIM7', colors:Array.from({length:16}, () => [90, 150, 210]),
            drums:{format, plugin, patch:patches[0], notes:Array.from({length:13}, (_, i) => 36 + i), percussion:[49]}
        });
        const [drum, first, second] = loads.splice(0);
        e2e.assert(first.instance !== second.instance, 'The grid pads must have independent instances');
        const originalIds = instanceIds();
        send(['secondary', 0], ['secondary', 3]);
        e2e.assert(loads.length === 0, 'An unassigned XY pad or fixed drum kit loaded a patch');
        send(['down', 1], ['up', 1], ['secondary', 2], ['secondary', 0]);
        send(['secondary', 2], ['secondary', 0]);
        const expected = [
            [second.instance, first.patch], [first.instance, second.patch],
            [second.instance, second.patch], [first.instance, first.patch]
        ];
        e2e.assert(e2e.same(loads.map(entry => [entry.instance, entry.patch]), expected),
            `Unexpected reroll targets or patches: ${JSON.stringify(loads)}`);
        e2e.assert(loads.every(entry => entry.created === false) && e2e.same(instanceIds(), originalIds),
            'Rerolling replaced an instance');

        const midi = [];
        sequencer.sendNoteOn = (instance, note) => { midi.push(['on', instance, note]); noteOn(instance, note); };
        sequencer.sendNoteOff = (instance, note) => { midi.push(['off', instance, note]); noteOff(instance, note); };
        const [chord] = cmrt_chord.note_progression('bIIIM7');
        e2e.assert(chord.length === 4, 'Expected four notes from bIIIM7');
        send(['down', 1], ['wheel', 1], ['up', 1], ['down', 2], ['wheel', 2], ['up', 2]);
        e2e.assert(e2e.same(midi, [
            ['on', first.instance, 60], ['off', first.instance, 60],
            ...chord.map(note => ['on', first.instance, note]),
            ...chord.map(note => ['off', first.instance, note]),
            ...chord.map(note => ['on', second.instance, note]),
            ...chord.map(note => ['off', second.instance, note]),
            ['on', second.instance, 60], ['off', second.instance, 60]
        ]), `Unexpected MIDI during wheel toggle: ${JSON.stringify(midi)}`);
        midi.length = 0;
        send(['wheel', 1], ['wheel', 3], ['wheel', 0], ['down', 3], ['up', 3], ['down', 0], ['up', 0], ['down', 1], ['up', 1]);
        e2e.assert(e2e.same(midi, [
            ['on', drum.instance, 36], ['off', drum.instance, 36],
            ['on', drum.instance, 60], ['off', drum.instance, 60],
            ...chord.map(note => ['on', first.instance, note]),
            ...chord.map(note => ['off', first.instance, note])
        ]), `Unexpected drum, XY, or instrument MIDI: ${JSON.stringify(midi)}`);
    } finally {
        sequencer.sendNoteOn = noteOn;
        sequencer.sendNoteOff = noteOff;
        repl.catalogInstruments = catalog;
        repl.patchState = patchState;
        repl.ui.surface.poll = poll;
        globalThis.performancePadSession?.();
    }
}
