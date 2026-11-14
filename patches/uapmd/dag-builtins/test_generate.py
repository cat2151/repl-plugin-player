"""Exercise deterministic generation and rejection without changing the checkout."""
import importlib.util
from pathlib import Path
import sys
import tempfile

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('repair', Path(__file__).with_name('generate.py'))
repair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(repair)
source = Path(sys.argv[1])
engine = Path(sys.argv[2]) if len(sys.argv) > 2 else None
engine_before = engine.read_bytes() if engine else None
before = source.read_bytes()
with tempfile.TemporaryDirectory(prefix='repl-dag-builtins-') as temporary:
    work = Path(temporary)
    output = work / 'generated/graph.cpp'
    repair.generate(source, output, engine_source=engine)
    generated = output.read_bytes()
    assert b'return repl_dag::appendEventsForGroup(dst, src, group);' in generated
    stamp = output.stat().st_mtime_ns
    repair.generate(source, output, engine_source=engine)
    assert output.read_bytes() == generated and output.stat().st_mtime_ns == stamp
    assert source.read_bytes() == before
    if engine:
        assert engine.read_bytes() == engine_before
        engine_output = output.parent / engine.name
        engine_generated = engine_output.read_bytes()
        assert b'repl_dag::processingGroup' in engine_generated
    drift = work / 'input/upstream.cpp'
    drift.parent.mkdir()
    drift.write_bytes(before + b'\n// changed\n')
    try:
        repair.generate(drift, output)
        raise AssertionError('accepted upstream drift')
    except ValueError as error:
        assert 'SHA256' in str(error)
    assert output.read_bytes() == generated
    if engine:
        drift_engine = work / 'input/SequencerEngine.cpp'
        drift_engine.write_bytes(engine_before + b'\n// changed\n')
        try:
            repair.generate(source, output, engine_source=drift_engine)
            raise AssertionError('accepted engine drift')
        except ValueError as error:
            assert 'SHA256' in str(error)
        assert output.read_bytes() == generated and engine_output.read_bytes() == engine_generated
print('PASS: deterministic, nonmutating generation; drift leaves previous output intact')
