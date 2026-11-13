// A chord progression becomes an SMF clip holding the converter's notes as
// two successive chords, every pitch reaches the plugin, and an invalid MIDI
// byte throws without breaking later callbacks. Test-owned script: snippet
// contents are never expectations.

// %% the converter's pitches for two chords of three notes
globalThis.source = 'F-G';
globalThis.expected = mmlabc_to_smf.pass3_events.ast_to_events(mmlabc_to_smf.pass2_ast.tokens_to_ast(
    mmlabc_to_smf.pass1_parser.parse_mml(cmrt_chord.resolve_chord_or_mml(source).mml)), true)
    .filter(e => e.type === 'note_on').map(e => e.note);
expected;

// %% generated paths - unique, and no file behind them
(() => {
    const paths = [getGeneratedMidiPath(), getGeneratedMidiPath()];
    e2e.assert(paths[0] !== paths[1], 'Generated paths collide');
    for (const path of paths)
        e2e.assert(!uapmd.timeline.addMidiClip(e2e.trackId(0), 0, path).success, `Path helper created a file: ${path}`);
    return paths;
})();

// %% generate the clip and play it
{
    const generatedTrack = 0;
    const midi = include('builtin/lib/midi.js');
    globalThis.generatedPath = midi.writeMml(cmrt_chord.resolve_chord_or_mml(source).mml);
    midi.addFile(generatedTrack, generatedPath);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}

// %% one clip with the converter's notes, as two successive chords
(() => {
    const clips = e2e.clips(0);
    e2e.assert(clips.length === 1, `Expected one musical clip: ${JSON.stringify(clips)}`);
    const notes = e2e.notes(clips[0].midi.events);
    const pitches = notes.map(n => n.pitch);
    e2e.assert(e2e.same(pitches, expected) && notes.every(n => n.end !== undefined),
        `Unexpected generated notes: ${JSON.stringify(notes)}`);
    e2e.assert(notes.slice(0, 3).every(n => n.start === notes[0].start)
        && notes.slice(3).every(n => n.start === notes[3].start)
        && notes[3].start > notes[0].start,
        'Chord progression was not two successive chords');
    // The clip's length at 120 BPM, and a margin.
    globalThis.playMs = Math.max(...notes.map(n => n.end)) / clips[0].midi.tickResolution * 500 + 300;
    return pitches;
})();

// %% every pitch reaches the plugin and sounds
(() => {
    const {peak, pitches} = e2e.heard(playMs);
    e2e.assert(peak > 0.001, `Generated clip did not sound: ${peak}`);
    e2e.assert(e2e.same(pitches, [...new Set(expected)].sort((a, b) => a - b)),
        `Not all generated pitches reached the plugin: ${JSON.stringify(pitches)}`);
    return {peak, pitches};
})();

// %% the generated SMF stays on disk
(() => {
    uapmd.sequencer.stopPlayback();
    const added = uapmd.timeline.addMidiClip(e2e.trackId(0), 0, generatedPath);
    e2e.assert(added.success, `Generated SMF was not retained: ${added.error}`);
    e2e.assert(uapmd.timeline.removeClip(added.clipId), 'E2E: could not remove the reloaded clip');
    return generatedPath;
})();

// %% error - an invalid MIDI byte throws
mmlabc_to_smf.pass4_midi.save_midi_file([256], getGeneratedMidiPath());

// %% the callback recovers
e2e.assert(cmrt_chord.resolve_chord_or_mml('cde').from_chord === false, 'Callback failed to recover');
source;
