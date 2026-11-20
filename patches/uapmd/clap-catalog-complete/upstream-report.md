# Newly installed CLAP plugins never appear once the plugin list cache has CLAP entries

`PluginScanToolImpl` skips enumerating a slow-scanning format's bundles when the
plugin list cache already holds any slow entry of that format and `forceRescan`
is false:

```cpp
if (!forceRescan && hasCachedSlowEntries)
    continue;
```

So after the first scan, a CLAP plugin installed later is not found by a normal
`performPluginScanning(false)` or by the startup fast scan. Only a forced rescan
(or deleting the cache) picks it up, and a forced rescan loads every bundle again.

Steps (Windows, revision `4ec5c3b1c30130fdd3b7d4b375082273247e561f`):

1. Scan once so that `plugin-list-cache.json` contains CLAP entries.
2. Remove one CLAP plugin's entries from the cache (equivalent to installing it afterwards).
3. Call `AppModel::performPluginScanning(false)` and wait for completion.
4. The plugin is still missing from the catalog and from the cache.

Suggestion: when the cache is used, still enumerate candidate bundles and scan
only those whose bundle path has no cached entry (blocklist excluded). Our host
does this after each scan as a workaround.
