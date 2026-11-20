#pragma once
#include <uapmd-plugin-hosting/uapmd-plugin-hosting.hpp>

namespace repl_clap_catalog {
// Scans the CLAP bundles that `host`'s catalog has no entry for and, when that adds
// plugins, rewrites the plugin list cache with them. True when the cache changed;
// the host's own catalog changes only once it is reloaded from the cache.
// Loads the new bundles' DLLs, so it belongs on the scan worker.
bool complete(uapmd_plugin_hosting::AudioPluginHostingAPI& host);
}
