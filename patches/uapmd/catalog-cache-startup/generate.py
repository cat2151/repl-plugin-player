"""Copy the upstream plugin host into the shim with a cache-only catalog load, rejecting changed upstream shapes."""
from pathlib import Path
import sys

HEADER = 'RemidyAudioPluginHost.hpp'
SOURCE = 'RemidyAudioPluginHost.cpp'


def replace_once(text, old, new, path):
    if text.count(old) != 1:
        raise RuntimeError(f'{path}: expected unique anchor: {old[:100]}')
    return text.replace(old, new)


def generate(upstream, directory):
    """`upstream` is the directory holding the host header and source."""
    header = upstream / HEADER
    source = upstream / SOURCE
    header_text = replace_once(
        header.read_text(encoding='utf-8'),
        '        void reloadPluginCatalogFromCache() override;\n',
        '        void reloadPluginCatalogFromCache() override;\n'
        '        bool replLoadCatalogFromCache();\n',
        header)
    source_text = source.read_text(encoding='utf-8')
    source_text = replace_once(
        source_text, '#include "RemidyAudioPluginHost.hpp"',
        '#include "' + (directory / HEADER).resolve().as_posix() + '"\n#include "' +
        (Path(__file__).parent / 'catalog_cache.hpp').resolve().as_posix() + '"', source)
    source_text = replace_once(
        source_text, '#include "RemidyAudioPluginInstance.hpp"',
        '#include "' + (upstream / 'RemidyAudioPluginInstance.hpp').resolve().as_posix() + '"', source)
    source_text = replace_once(
        source_text, 'void uapmd_plugin_hosting::RemidyAudioPluginHost::reloadPluginCatalogFromCache() {',
        '''bool uapmd_plugin_hosting::RemidyAudioPluginHost::replLoadCatalogFromCache() {
    auto& cacheFile = scanning->pluginListCacheFile();
    if (cacheFile.empty() || !std::filesystem::exists(cacheFile))
        return false;
    scanning->catalog().clear();
    try {
        scanning->catalog().load(cacheFile);
    } catch (...) {
        scanning->catalog().clear();
        return false;
    }
    if (scanning->catalog().getPlugins().empty()) {
        scanning->catalog().clear();
        return false;
    }
    return true;
}

bool repl_catalog_cache::load(uapmd_plugin_hosting::AudioPluginHostingAPI& host) {
    auto* remidyHost = dynamic_cast<uapmd_plugin_hosting::RemidyAudioPluginHost*>(&host);
    return remidyHost && remidyHost->replLoadCatalogFromCache();
}

void uapmd_plugin_hosting::RemidyAudioPluginHost::reloadPluginCatalogFromCache() {''', source)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / HEADER).write_text(header_text, encoding='utf-8')
    (directory / SOURCE).write_text(source_text, encoding='utf-8')


if __name__ == '__main__':
    generate(Path(sys.argv[1]), Path(sys.argv[2]))
