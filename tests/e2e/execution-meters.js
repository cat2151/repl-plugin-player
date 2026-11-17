// Test-owned inputs: instance totals must never collapse equal pitches.
// %% setup
{
    const plugins = include('builtin/lib/plugins.js');
    globalThis.meterFixture = {first: plugins.instrumentOn(0)};
    const entry = plugins.catalogEntry('CLAP', 'Six Sines');
    meterFixture.second = uapmd.instancing.create(entry.format, entry.pluginId);
    if (meterFixture.second < 0) throw new Error('second instrument failed');
    meterFixture.effect = uapmd.instancing.create('CLAP', 'michaelwillis.dragonfly.hall',
        include('builtin/lib/tracks.js').idOf(0));
    if (meterFixture.effect < 0) throw new Error('effect failed');
    include('builtin/lib/effect.js').joinBeforeOutput(0, meterFixture.effect);
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}
// %% same pitch on two instruments
{
    uapmd.sequencer.sendNoteOn(meterFixture.first, 60);
    uapmd.sequencer.sendNoteOn(meterFixture.second, 60);
}
// %% repeated note and effect input
{
    uapmd.sequencer.sendNoteOn(meterFixture.first, 60);
    uapmd.sequencer.sendNoteOn(meterFixture.effect, 60);
}
// %% release once
uapmd.sequencer.sendNoteOff(meterFixture.first, 60);
// %% release second instrument
uapmd.sequencer.sendNoteOff(meterFixture.second, 60);
// %% release remaining
uapmd.sequencer.sendNoteOff(meterFixture.first, 60);
// %% observe
repl.test.observe()
// %% stop
include('builtin/lib/tutorial.js').stop();
// %% restart
{
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}
