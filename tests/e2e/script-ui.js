// `script-ui`: a script window's slider moves a real plugin parameter through
// egui and follows outside changes. The checks are Rust
// (`src/script_ui/regression.rs`); it runs these cells by title.

// %% unrelated
throw new Error('unrelated cell');

// %% open
// One slider bound to a writable continuous parameter of track 0's instrument.
(() => {
    var id = include('builtin/lib/plugins.js').instrumentOn(0);
    var parameter = uapmd.instance(id).getParameters().find(p => !p.isReadonly && !p.isDiscrete &&
        Number.isInteger(p.id) && p.id >= 0 && Number.isFinite(p.minValue) && Number.isFinite(p.maxValue) &&
        p.minValue < p.maxValue);
    if (!parameter) throw new Error('E2E: no writable continuous parameter');
    repl.ui.show({id:'e2e-parameters', title:'E2E', widgets:[
        {id:'parameter', type:'slider', label:parameter.name, binding:{instanceId:id, parameterId:parameter.id}}
    ]});
})();

// %% close
repl.ui.close('e2e-parameters');

// %% set parameter
({instance, parameter, value}) => uapmd.instance(instance).setParameterValue(parameter, value)
