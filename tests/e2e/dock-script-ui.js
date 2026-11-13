// `dock-script-ui`: a `repl.ui` window starts in its own window, moves into the tree
// and back, moves a real plugin parameter as a tab, and closes from either place. The
// checks are Rust (`src/gui/dock/script_regression.rs`); it runs these cells by title.

// %% open
// One slider bound to a writable continuous parameter of track 0's instrument.
{
    const id = include('builtin/lib/plugins.js').instrumentOn(0);
    const parameter = uapmd.instance(id).getParameters().find(p => !p.isReadonly && !p.isDiscrete &&
        Number.isInteger(p.id) && p.id >= 0 && Number.isFinite(p.minValue) && Number.isFinite(p.maxValue) &&
        p.minValue < p.maxValue);
    if (!parameter) throw new Error('E2E: no writable continuous parameter');
    repl.ui.show({id:'e2e-dock', title:'E2E dock', widgets:[
        {id:'parameter', type:'slider', label:parameter.name, binding:{instanceId:id, parameterId:parameter.id}}
    ]});
}

// %% close
repl.ui.close('e2e-dock');
