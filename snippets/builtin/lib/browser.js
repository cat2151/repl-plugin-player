// The player's patch or effect browser as a loop. Loading only defines functions.
(() => {
    // Opens the browser with `options` (repl.patchBrowser.open's) and hands each
    // selection to `apply`; its error goes back to the player, which shows it.
    // Returns when the browser is closed.
    function run(options, apply) {
        repl.patchBrowser.open(options);
        for (;;) {
            repl.processEvents();
            for (const event of repl.patchBrowser.poll()) {
                if (event.kind === 'closed') {
                    if (event.error) throw new Error(event.error);
                    return;
                }
                let error;
                try {
                    apply(event);
                } catch (e) {
                    error = e instanceof Error ? e.message : String(e);
                }
                repl.patchBrowser.answer(error);
            }
        }
    }

    return {run};
})();
