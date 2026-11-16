(() => {
    function create(progression, {inversions, octaves, maxLeap, range}) {
        if (range && !(Number.isFinite(range.min) && Number.isFinite(range.max)
            && range.min <= range.low && range.low <= range.high && range.high <= range.max))
            throw new Error('Expected min <= low <= high <= max for voicing range');
        const chords = progression.split('-').map(chord => chord.trim());
        const candidates = chords.map(chord => inversions.flatMap(inversion => octaves.map(octave => {
            const notation = chord + (inversion ? '^' + inversion : '')
                + (octave < 0 ? ','.repeat(-octave) : "'".repeat(octave));
            const parsed = cmrt_chord.note_progression(notation);
            if (parsed.length !== 1) throw new Error('Expected one chord: ' + notation);
            return {notation, inversion, octave, notes:parsed[0].slice().sort((a,b) => a-b)};
        })).filter(c => !range || (c.notes[0] >= range.min && c.notes[c.notes.length-1] <= range.max)));
        let current = [];
        function next(pin = null) {
            // An exponential race gives a weighted random permutation. Draw once
            // per candidate, never inside sort's comparator. Distance penalizes
            // excursions quadratically, so the pull back grows farther out.
            const ranked = candidates.map(options => options.map(candidate => {
                const below = range ? Math.max(0, range.low - candidate.notes[0]) : 0;
                const above = range ? Math.max(0, candidate.notes[candidate.notes.length-1] - range.high) : 0;
                const penalty = range ? (below*below + above*above) / (12*12) : 0;
                return {candidate, score:Math.log(-Math.log(1 - Math.random())) + penalty};
            }).sort((a,b) => a.score - b.score).map(entry => entry.candidate));
            const path = repl.sequence.choose(ranked.map(options => options.map(c => c.notes)),
                current.map(c => c.notes), maxLeap, pin);
            current = path.map((choice,i) => ranked[i][choice]);
            return {progression:current.map(c => c.notation).join('-'), steps:current.map(c => c.notes)};
        }
        return {next};
    }
    return {create};
})();
