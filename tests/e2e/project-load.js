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
!globalThis.loopParts && !uapmd.timeline.getState().loopEnabled;

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
