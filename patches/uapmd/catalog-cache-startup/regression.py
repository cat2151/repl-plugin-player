"""Generation contracts on the real upstream host source, outside the repo."""
import importlib.util
from pathlib import Path
import shutil
import tempfile

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('catalog_cache_generator', HERE / 'generate.py')
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)
upstream = HERE.parents[3] / 'uapmd/source/uapmd-plugin-hosting/src/plugin-api'


def rejected(directory, output):
    try:
        generator.generate(directory, output)
    except RuntimeError:
        return True
    return False


with tempfile.TemporaryDirectory(prefix='repl-catalog-cache-') as temp:
    root = Path(temp)
    output = root / 'shim'
    for _ in range(2):  # Each configure starts from upstream, even with a cache.
        generator.generate(upstream, output)
        header = (output / generator.HEADER).read_text(encoding='utf-8')
        source = (output / generator.SOURCE).read_text(encoding='utf-8')
        assert header.count('bool replLoadCatalogFromCache();') == 1
        assert source.count('RemidyAudioPluginHost::replLoadCatalogFromCache() {') == 1
        assert source.count('bool repl_catalog_cache::load(') == 1
        assert (output / generator.HEADER).resolve().as_posix() in source
        assert (upstream / 'RemidyAudioPluginInstance.hpp').resolve().as_posix() in source
    # An already generated copy has no upstream anchors left.
    assert rejected(output, root / 'again')
    for name, old in [(generator.HEADER, 'void reloadPluginCatalogFromCache() override;'),
                      (generator.SOURCE, 'RemidyAudioPluginHost::reloadPluginCatalogFromCache() {')]:
        changed = root / ('changed-' + name)
        shutil.copytree(upstream, changed)
        text = (changed / name).read_text(encoding='utf-8')
        (changed / name).write_text(text.replace(old, old.replace('reload', 'refresh')), encoding='utf-8')
        assert rejected(changed, root / 'out'), f'Changed upstream {name} accepted'
print('PASS catalog-cache-startup generation, fresh/cache, unknown anchors')
