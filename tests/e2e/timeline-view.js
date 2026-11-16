// `timeline-view`: what the piano rolls show for tracks, clips and a loop.
// The checks are Rust (`src/timeline/regression.rs`, the projection and the
// native view); it runs these cells by title.

// %% add a track
globalThis.timelineViewJob = uapmd.sequencer.addTrack();

// %% remove track 1
globalThis.timelineViewJob = uapmd.sequencer.removeTrack(include('builtin/lib/tracks.js').idOf(1));

// %% job
uapmd.mutations.getJob(timelineViewJob.jobId)

// %% chord at beat one
var tvClip = uapmd.timeline.createEmptyMidiClip(include('builtin/lib/tracks.js').idOf(0), uapmd.sequencer.getSampleRate()/2, 480, 120);
uapmd.timeline.addUmpEvent(tvClip,0,[0x20903c64]);
uapmd.timeline.addUmpEvent(tvClip,0,[0x20904064]);
uapmd.timeline.addUmpEvent(tvClip,960,[0x20803c00]);
uapmd.timeline.addUmpEvent(tvClip,960,[0x20804000]);

// %% loop on track 0
// A test-owned phrase, independent of the user's editable built-in snippets.
var tvPath = getGeneratedMidiPath();
mmlabc_to_smf.pass4_midi.save_midi_file(mmlabc_to_smf.mml_to_smf_bytes('l4 >cg<cg'),tvPath);
include('builtin/lib/midi.js').addFile(0,tvPath);
repl.timeline.setLoop(0, uapmd.sequencer.getSampleRate()*2);
uapmd.audio.setEngineEnabled(true); uapmd.sequencer.startPlayback();

// %% clear the loop
repl.timeline.clearLoop()

// %% loop on track 1
// Using an initially empty track for a loop also makes it a user-used track.
include('builtin/lib/midi.js').addFile(1,tvPath);
repl.timeline.setLoop(0, uapmd.sequencer.getSampleRate()*2);

// %% save and reload loop
{
    const path = getGeneratedMidiPath() + '.uapmdz';
    repl.project.saveArchive(path);
    repl.timeline.clearLoop();
    repl.project.load(path);
}

// %% play restored loop
uapmd.audio.setEngineEnabled(true);
uapmd.sequencer.startPlayback();

// %% stop restored loop
uapmd.sequencer.stopPlayback();
uapmd.audio.setEngineEnabled(false);

// %% live sequence projection
repl.sequence.set({track:0,steps:[[60,64],[62,65]],beats:4,bpm:120,gate:1,velocity:18000});

// %% update live sequence projection
repl.sequence.set({track:0,steps:[[67,71],[69,72]],beats:4,bpm:120,gate:1,velocity:18000});

// %% clear live sequence projection
repl.sequence.clear();
