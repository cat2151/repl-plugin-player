"""Check upstream anchors, idempotent output and fail-closed drift detection."""
from pathlib import Path
import runpy
import sys

module = runpy.run_path(str(Path(__file__).with_name('generate.py')))
transform=module['transform']
source = Path(sys.argv[1]).read_text(encoding='utf-8')
result = transform(source)
assert result.count('processGraph ? processingGraph : nullptr') == 1
assert 'processingGraph->processAudio(tp)' in result
assert 'repl_evolution_graphs::workerBudget()' in result
assert transform(source) == result
for changed in [result, source.replace('job = {processGraph ?', 'job = {changed ?'), source + source]:
    try:
        transform(changed)
    except ValueError:
        pass
    else:
        raise AssertionError('Changed or already patched source was accepted')
print('PASS evolution-graphs generation and drift guards')

if len(sys.argv)>2:
    workers=Path(sys.argv[2])
    outputs=module['worker_outputs'](workers)
    assert outputs['AudioTrackWorkerPool.hpp'].count('repl_evolution_graphs::pollWorkers();')==1
    result=transform(source,Path('X:/generated/AudioWorkers.hpp'))
    assert '#include "X:/generated/AudioWorkers.hpp"' in result
    assert 'if (!offline_rendering_.load(std::memory_order_acquire)) repl_evolution_graphs::pollWorkers();' in result
    print('PASS evolution-graphs private worker/coordinator hooks')
