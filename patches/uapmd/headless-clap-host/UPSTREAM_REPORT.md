# CLAP value rescan walks every parameter on the main thread

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告; draft only.

`RemidyCLAPHost::paramsRescan(CLAP_PARAM_RESCAN_VALUES)` calls
`ParameterSupport::parameters()` and then `getParameter` plus
`notifyParameterValue` for every parameter. On an instance whose list was not yet
built, `parameters()` first runs `populateParameterList`, which calls
`paramsGetInfo` for each parameter and probes `paramsValueToText` for stepped ones.

Six Sines calls `params->rescan` from `stateLoad` and again from `onMainThread`
after `request_callback`. With its 2554 parameters, one `loadState` blocks the main
thread for about 1.5 s, and for about 4 s on a fresh instance (most of it in
`populateParameterList`). Other hosts load the same state in well under a second.

The app works around this only for instances it never exposes to UI or scripts:
they get a host subclass whose `paramsRescan` does nothing. A general fix could be
one of:

- skip the value walk while the parameter list has not been built yet (no observer
  can hold old values);
- mark values dirty on rescan and refresh them lazily when they are read or
  displayed;
- let the host creator opt out of parameter observation per instance.

Regression: loading a Six Sines state into an existing instance should not block
the main thread for longer than the plugin's own `stateLoad`.
