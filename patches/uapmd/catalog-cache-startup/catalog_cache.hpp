#pragma once
#include <uapmd-plugin-hosting/uapmd-plugin-hosting.hpp>

namespace repl_catalog_cache {
// Replaces the host's catalog with the plugin list cache, without enumerating any
// format. False when there is no cache, it has no plugins, or it cannot be read;
// the catalog is then empty and a scan has to fill it.
bool load(uapmd_plugin_hosting::AudioPluginHostingAPI& host);
}
