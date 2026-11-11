// @text comment_05
var snippetTrack = include('builtin/lib/plugins.js').trackFor(globalThis.surgeId, 'snippet_00');
var {clipId:snippetClip, tickResolution:snippetTpqn} = include('builtin/lib/midi.js').addNoteClip(snippetTrack.trackIndex,
    {bpm:120, pitch:60, velocity:0x7fff, start:1, length:2});
