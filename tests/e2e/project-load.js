// Test-owned inputs; builtin snippet contents remain editable.
// %% load
repl.project.load(projectPath);

// %% change
{
    const melodyTrack = 0;
    repl.track.setVolume(melodyTrack, 0.13);
    include('builtin/lib/tutorial.js').replaceMml(melodyTrack, 'gaf', 123);
}

// %% prepare outgoing loop
{
    const melodyTrack = 0;
    const loop = include('builtin/lib/loop.js');
    loop.publishLoopParts({[melodyTrack]:loop.prepareLoopPart(melodyTrack, 'gab')}, 120);
}

// %% outgoing loop cleared
(() => {
    if (globalThis.loopParts) return 'loopParts retained';
    try {
        // A new publication must configure timing instead of inheriting the old loop.
        repl.timeline.publishLoopClips([]);
    } catch (error) {
        return String(error).includes('Configure the common loop first') || String(error);
    }
    return 'old timing retained';
})();

// %% invalid source
(() => {
    for (const path of ['', projectPath + '.missing', invalidProjectPath]) {
        try {
            repl.project.load(path);
            return false;
        } catch (error) {
            if (!String(error).includes('project.load:')) return false;
        }
    }
    return true;
})();

// %% snippet smoke
include('builtin/load-project.js');
