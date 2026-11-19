"""Run: python -B patches/uapmd/parameter-value-sweeps/test_apply.py"""
import importlib.util
from pathlib import Path
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCES = (ROOT / "target/shim/generated/clap-note-dialect/HostClasses.cpp",
           ROOT.parent / "uapmd/source/uapmd-engine/src/sequencer/TimelineFacadePlugins.cpp")
spec = importlib.util.spec_from_file_location("repair", HERE / "apply.py")
repair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(repair)


def check(host, timeline):
    """No value walk remains in either function, and each replacement is there once."""
    rescan = host[host.index("RemidyCLAPHost::paramsRescan"):host.index("RemidyCLAPHost::paramsClear")]
    assert "getParameter(" not in rescan and "notifyParameterValue" not in rescan, "rescan still walks values"
    assert "refreshAllParameterMetadata" in rescan, "metadata rescans were removed"
    cache = timeline[timeline.index("TimelineFacadeImpl::refreshPluginParameterCache"):]
    cache = cache[:cache.index("\n    }\n")]
    assert "getParameterValue" not in cache, "parameter cache still asks the plugin"
    assert host.count(repair.RESCAN_REPLACEMENT) == 1 and timeline.count(repair.CACHE_REPLACEMENT) == 1


def fails(texts, work):
    paths = []
    for source, text in zip(SOURCES, texts):
        path = work / "bad" / source.name
        path.parent.mkdir(exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")
        paths.append(path)
    try:
        repair.generate(*paths, work / "bad-out")
    except ValueError:
        return True
    return False


originals = [source.read_text(encoding="utf-8") for source in SOURCES]
with tempfile.TemporaryDirectory() as directory:
    work = Path(directory)
    inputs = []
    for source, text in zip(SOURCES, originals):
        path = work / "in" / source.name
        path.parent.mkdir(exist_ok=True)
        path.write_text(text.replace("\n", "\r\n"), encoding="utf-8", newline="")
        inputs.append(path)
    before = [path.read_bytes() for path in inputs]
    outputs = repair.generate(*inputs, work / "out")
    check(*(path.read_text(encoding="utf-8") for path in outputs))
    assert [path.read_bytes() for path in inputs] == before, "input changed"
    stamps = [path.stat().st_mtime_ns for path in outputs]
    assert repair.generate(*inputs, work / "out") == outputs, "reapply moved the output"
    assert [path.stat().st_mtime_ns for path in outputs] == stamps, "reapply rewrote"
    # Mutation: the unpatched sources must fail the same check.
    try:
        check(*originals)
    except AssertionError:
        pass
    else:
        raise AssertionError("check accepts the unpatched sources")
    host, timeline = originals
    for anchor, index in ((repair.RESCAN_ANCHOR, 0), (repair.CACHE_ANCHOR, 1)):
        for mutated in ("", anchor * 2):
            texts = list(originals)
            texts[index] = texts[index].replace(anchor, mutated)
            assert fails(texts, work), f"anchor {index} {'missing' if not mutated else 'repeated'} accepted"
print("parameter-value-sweeps: ok")
