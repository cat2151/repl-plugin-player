"""Generation contracts on pristine and existing-cache inputs, outside the repo."""
import importlib.util
from pathlib import Path
import tempfile

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('metadata_generator', HERE / 'generate.py')
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)
upstream = HERE.parents[3] / 'uapmd/source/remidy/src/clap/PluginInstanceCLAP.Parameters.cpp'
runtime = HERE.parents[3] / 'uapmd/source/tools/uapmd-app-model/src/UapmdJSRuntime.cpp'
with tempfile.TemporaryDirectory(prefix='repl-metadata-') as temp:
    root = Path(temp)
    target = root / upstream.name
    for _ in range(2):  # Each configure starts from upstream, even with a cache.
        target.write_bytes(upstream.read_bytes())
        generator.generate(root)
        result = target.read_text(encoding='utf-8')
        assert 'repl_parameters::report' in result and 'info.flags' in result
        assert 'repl_parameters::invalidate' in result and 'repl_parameters::destroy' in result
    try:
        generator.generate(root)
    except RuntimeError:
        pass
    else:
        raise AssertionError('Already patched/unknown source accepted')
    target.write_text(upstream.read_text(encoding='utf-8').replace('return parameter_defs;', 'return changed;'), encoding='utf-8')
    try:
        generator.generate(root)
    except RuntimeError:
        pass
    else:
        raise AssertionError('Changed upstream accepted')
    js = root / 'runtime.cpp'
    js.write_bytes(runtime.read_bytes())
    generator.runtime_metadata(js)
    assert '"isReadonly", false' not in js.read_text(encoding='utf-8')
print('PASS parameter-metadata generation, fresh/cache, unknown anchors, real JS readonly')
