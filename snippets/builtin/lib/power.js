// Windows' "minimum processor state", raised now and written back when the app ends.
(() => {
    const title = () => repl.text('min_processor_state_title');

    // Sets `percent` now, and `restorePercent` when the app ends normally. A crash or
    // a kill leaves `percent`; choosing the snippet again with `percent` equal to
    // `restorePercent` sets it back and writes nothing at the end.
    function setMinProcessorState(percent, restorePercent) {
        if (![percent, restorePercent].every(p => Number.isInteger(p) && p >= 0 && p <= 100))
            throw new Error(repl.text('min_processor_state_range'));
        repl.power.setMinProcessorState(percent);
        if (percent === restorePercent) {
            repl.app.onExit('minProcessorState', null);
            return;
        }
        repl.app.onExit('minProcessorState', () => {
            let text;
            try {
                repl.power.setMinProcessorState(restorePercent);
                text = repl.text('min_processor_state_restored', {restore:restorePercent});
            } catch (error) {
                text = repl.text('min_processor_state_restore_failed', {restore:restorePercent, error:error.message});
            }
            repl.ui.dialog(title(), text);
        });
        repl.ui.dialog(title(), repl.text('min_processor_state_raised', {percent, restore:restorePercent}));
    }

    return {setMinProcessorState};
})();
