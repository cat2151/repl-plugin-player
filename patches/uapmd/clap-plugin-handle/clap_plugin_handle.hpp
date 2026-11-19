#pragma once

namespace remidy { class PluginInstance; }
namespace uapmd_plugin_hosting { class AudioPluginInstanceAPI; }

namespace repl_clap_plugin_handle {
    // The instance's `clap_plugin*`, or nullptr when it is not a remidy CLAP instance.
    // Valid while the instance lives; the plugin is called on its main thread only.
    const void* of(uapmd_plugin_hosting::AudioPluginInstanceAPI* instance);
    // The same for remidy's instance; built in the remidy target.
    const void* ofRemidy(remidy::PluginInstance* instance);
}
