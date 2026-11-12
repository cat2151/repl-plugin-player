// %% @text name_try_instrument
// @text demo_reset
{
    const instrumentTrack = 2;
    const demo = include('builtin/lib/tutorial.js');
    demo.reset();
    const synth = demo.prepareInstrument(instrumentTrack, 'CLAP', 'Six Sines', false);
    const mml = 'r c r c r c';
    const clip = demo.replaceMml(synth.trackIndex, mml, 120);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
    ({...synth, ...clip});
}
