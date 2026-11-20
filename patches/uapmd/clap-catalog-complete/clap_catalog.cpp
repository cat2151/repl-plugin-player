#include "clap_catalog.hpp"
#include <remidy/remidy.hpp>
#include <algorithm>

using namespace uapmd_plugin_hosting;

bool repl_clap_catalog::complete(AudioPluginHostingAPI& host) {
    auto entries = host.pluginCatalogEntries();
    auto tool = PluginScanTool::create();
    bool changed = false;
    for (auto* format : host.pluginFormats()) {
        if (format->name() != "CLAP")
            continue;
        auto* scanner = dynamic_cast<AudioPluginFileOrUrlScanning*>(format->scanning());
        if (!scanner)
            continue;
        // Populates the upstream scanner's pending list, including Windows DLL files.
        scanner->getAllFastScannablePlugins();
        for (const auto& bundle : scanner->enumerateCandidateBundles(false)) {
            if (tool->isBundleBlocklisted("CLAP", bundle))
                continue;
            if (std::any_of(entries.begin(), entries.end(), [&](const auto& entry) {
                return entry.format() == "CLAP" && entry.bundlePath() == bundle;
            }))
                continue;
            std::string error;
            scanner->scanBundle(bundle, false, 0.0, [&](AudioPluginCatalogEntry entry) {
                if (std::none_of(entries.begin(), entries.end(), [&](const auto& existing) {
                    return existing.format() == entry.format() && existing.pluginId() == entry.pluginId();
                })) {
                    entries.push_back(std::move(entry));
                    changed = true;
                }
            }, [&](std::string message) { error = std::move(message); });
            if (!error.empty())
                remidy::Logger::global()->logWarning("CLAP scan: %s", error.c_str());
        }
    }
    if (!changed || tool->pluginListCacheFile().empty())
        return false;
    AudioPluginCatalog catalog;
    for (const auto& entry : entries)
        if (entry.format() == "CLAP" || entry.format() == "VST3")
            catalog.add(entry);
    catalog.save(tool->pluginListCacheFile());
    return true;
}
