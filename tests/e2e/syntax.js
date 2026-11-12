// A Run with a syntax error executes none of it.

// %% error - an engine enable before a syntax error
uapmd.audio.setEngineEnabled(true); const = ;

// %% the engine stays disabled
{
    const {engineEnabled} = repl.test.observe();
    e2e.assert(engineEnabled === false, 'Syntax error executed the preceding engine enable');
    engineEnabled;
}
