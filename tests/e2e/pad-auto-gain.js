// Use a test-owned quiet synth and measure the actual UAPMD master output.
// %% quiet instrument
globalThis.padGainFixture = (() => {
    const track = 0;
    const {instanceId} = include('builtin/lib/patch.js').use({
        format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', track
    });
    const synth = uapmd.instance(instanceId);
    const volume = synth.getParameters().find(p => p.name === 'Global Volume');
    e2e.assert(volume, 'Missing Surge XT Global Volume');
    uapmd.audio.setEngineEnabled(true);
    e2e.wait(250);
    synth.setParameterValue(volume.id, 0.70);
    e2e.wait(250);
    return {instanceId};
})();

// %% boost the first held chord and the next held single note
{
    const {instanceId} = padGainFixture;
    const single = [60];
    const chord = [60, 64, 67];
    function heldPeak(notes) {
        e2e.assert(e2e.waitFor(() => repl.test.observe().peak < 0.0001, 5000), 'Release never went quiet');
        for (const note of notes) uapmd.sequencer.sendNoteOn(instanceId, note);
        const {peak} = e2e.observe(350);
        for (const note of notes) uapmd.sequencer.sendNoteOff(instanceId, note);
        return peak;
    }
    const singleBaseline = heldPeak(single);
    const chordBaseline = heldPeak(chord);
    for (const peak of [singleBaseline, chordBaseline])
        e2e.assert(peak > 0.001 && peak < 0.15, `Unexpected baseline ${peak}`);
    repl.audio.setPadAutoGain([instanceId]);
    const firstChord = heldPeak(chord);
    const nextSingle = heldPeak(single);
    // Start a single note while the chord is still releasing.
    for (const note of chord) uapmd.sequencer.sendNoteOn(instanceId, note);
    e2e.wait(350);
    for (const note of chord) uapmd.sequencer.sendNoteOff(instanceId, note);
    for (const note of single) uapmd.sequencer.sendNoteOn(instanceId, note);
    e2e.wait(1200);
    const {peak:overTail} = e2e.observe(350);
    for (const note of single) uapmd.sequencer.sendNoteOff(instanceId, note);
    repl.audio.setPadAutoGain([]);
    const bypass = heldPeak(single);
    e2e.assert(firstChord > chordBaseline * 1.5, `First chord: ${chordBaseline} -> ${firstChord}`);
    const chordGain = firstChord / chordBaseline;
    const singleGain = nextSingle / singleBaseline;
    e2e.assert(singleGain > chordGain * 1.25, `Boost did not recover: ${chordGain} -> ${singleGain}`);
    e2e.assert(overTail > nextSingle * 0.9, `Stuck under the chord tail: ${overTail} < ${nextSingle}`);
    e2e.assert(bypass > singleBaseline * 0.8 && bypass < singleBaseline * 1.2,
        `Bypass: ${singleBaseline} -> ${bypass}`);
    ({singleBaseline, chordBaseline, firstChord, nextSingle, overTail, bypass});
}
