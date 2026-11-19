# Loading a plugin state walks every parameter value three times on the main thread

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告; draft only.

Loading a state into a track's CLAP plugin reads every parameter value on the main
thread in three places:

1. `RemidyCLAPHost::paramsRescan(CLAP_PARAM_RESCAN_VALUES)` calls `getParameter`
   and `notifyParameterValue` for each parameter. Six Sines rescans from
   `stateLoad` and again from `onMainThread`, so this runs twice per load.
2. `TimelineFacadeImpl::refreshPluginParameterCache`, called from
   `finishPluginMutation`, calls `getParameterValue` for each parameter, twice
   (metadata indices, then every cached index).

The cache filled by 2 (`plugin_parameter_values_`) is written and erased but never
read, and value readers (`getParameterValue`) ask the plugin directly. With Six
Sines (2554 parameters, about 0.29 ms per `params.get_value`) one state load into
an existing instance blocks the main thread for about 3 s; the plugin's own
`stateLoad` takes a few tens of milliseconds.

The app removes the value branch of `paramsRescan` and makes
`refreshPluginParameterCache` drop the instance's entries. Possible upstream fixes:

- treat `CLAP_PARAM_RESCAN_VALUES` as "values may have changed" and let readers
  query on demand (or refresh lazily when a value is displayed);
- remove `plugin_parameter_values_` or fill it only for indices that are read.

Regression: loading a Six Sines state into an existing track instance should not
block the main thread for much longer than the plugin's own `stateLoad`, and
`getParameterValue` afterwards should return the loaded values.
