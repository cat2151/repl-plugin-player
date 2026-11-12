// `state-load`: `loadState(file)` from a cell returns and restores the whole
// state; an unreadable file fails as `{success:false}`. The checks are Rust
// (`state_regression.rs` beside this file, which writes the state file); it
// runs these cells by title.

// %% instrument
include('builtin/lib/plugins.js').instrumentOn(0)

// %% engine on
uapmd.audio.setEngineEnabled(true)

// %% continuous parameter
({instance}) => uapmd.instance(instance).getParameters().find(p => !p.isReadonly && !p.isDiscrete && p.maxValue > p.minValue).id

// %% parameter value
({instance, parameter}) => uapmd.instance(instance).getParameterValue(parameter)

// %% set parameter
({instance, parameter, value}) => uapmd.instance(instance).setParameterValue(parameter, value)

// %% load
({instance, file}) => uapmd.instance(instance).loadState(file)
