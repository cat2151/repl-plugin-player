// %% @text name_try_cut_paste
// @text demo_reset
{
    const synthTrack = 0;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    // @text demo_levels
    const synth = demo.prepareInstrument({format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp', track:synthTrack});
    const mml = 'r cfg';
    const clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    ({...synth, ...clip});
}
// %% @text cut_paste_cut
{
    const synthTrack = 0;
    const cutPitch = 60;
    const demo = include('builtin/lib/tutorial.js');
    const [clip] = uapmd.timeline.getClips(include('builtin/lib/tracks.js').idOf(synthTrack));
    const cut = uapmd.timeline.getClipNotes(clip.clipId).notes.filter(n => n.note === cutPitch);
    repl.pianoRoll.cut(clip.clipId, cut.map(n => n.noteId));
    demo.stop();
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    uapmd.timeline.getClipNotes(clip.clipId).notes.map(n => n.note);
}
// %% @text cut_paste_paste
{
    const synthTrack = 0;
    const afterPitch = 67;
    const demo = include('builtin/lib/tutorial.js');
    const [clip] = uapmd.timeline.getClips(include('builtin/lib/tracks.js').idOf(synthTrack));
    // @text cut_paste_tick
    const after = uapmd.timeline.getClipNotes(clip.clipId).notes.find(n => n.note === afterPitch);
    repl.pianoRoll.paste(clip.clipId, after.endTick);
    demo.stop();
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    uapmd.timeline.getClipNotes(clip.clipId).notes.map(n => n.note);
}
