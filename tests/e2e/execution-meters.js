// Test-owned inputs for the load meter's timing telemetry.
// %% setup
{
    const plugins = include('builtin/lib/plugins.js');
    globalThis.meterFixture = {instrument: plugins.instrumentOn(0)};
    uapmd.audio.setEngineEnabled(true);
    uapmd.sequencer.startPlayback();
}
// %% observe
repl.test.observe()
// %% stop
include('builtin/lib/tutorial.js').stop();
