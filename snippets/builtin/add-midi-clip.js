// @text comment_05
{
    const clipTrack = 0;
    include('builtin/lib/midi.js').addNoteClip(clipTrack,
        {bpm:120, pitch:60, velocity:0x7fff, start:1, length:2});
}
