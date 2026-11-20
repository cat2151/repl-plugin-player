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
assert 'repl_evolve_graphs::workerBudget()' in result
assert transform(source) == result
for changed in [result, source.replace('job = {processGraph ?', 'job = {changed ?'), source + source]:
    try:
        transform(changed)
    except ValueError:
        pass
    else:
        raise AssertionError('Changed or already patched source was accepted')
print('PASS evolve-graphs generation and drift guards')
