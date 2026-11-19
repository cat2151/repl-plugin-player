"""Run: python -B patches/uapmd/headless-clap-host/test_apply.py [--source <clap-note-dialect PluginFormatCLAP.cpp>]"""
import argparse
import importlib.util
from pathlib import Path
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path,
                    default=ROOT / "target/shim/generated/clap-note-dialect/PluginFormatCLAP.cpp")
args = parser.parse_args()
spec = importlib.util.spec_from_file_location("repair", HERE / "apply.py")
repair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(repair)


def check(text):
    """The generated source must pick the host by the scope depth, exactly once."""
    assert text.count("std::make_unique<HeadlessCLAPHost>()") == 1, "headless host is not selectable"
    assert text.count("repl_headless::depth > 0") == 1, "selection does not read the scope depth"
    assert repair.ANCHOR not in text, "unconditional RemidyCLAPHost remains"
    for name in ("headless_scope.hpp", "headless_host.hpp"):
        include = f'#include "{(HERE / name).as_posix()}"'
        assert text.count(include) == 1 and text.index(include) < text.index("\nnamespace remidy {"), name


def fails(text, work):
    source = work / "PluginFormatCLAP.cpp"
    source.write_text(text, encoding="utf-8", newline="\n")
    try:
        repair.generate(source, work / "out")
    except ValueError:
        return True
    return False


original = args.source.read_text(encoding="utf-8")
with tempfile.TemporaryDirectory() as directory:
    work = Path(directory)
    source = work / "in" / "PluginFormatCLAP.cpp"
    source.parent.mkdir()
    source.write_text(original.replace("\n", "\r\n"), encoding="utf-8", newline="")
    before = source.read_bytes()
    path = repair.generate(source, work / "out")
    text = path.read_text(encoding="utf-8")
    check(text)
    assert source.read_bytes() == before, "input changed"
    stamp = path.stat().st_mtime_ns
    assert repair.generate(source, work / "out") == path and path.stat().st_mtime_ns == stamp, "reapply rewrote"
    # Mutation: the unpatched source must fail the same check.
    try:
        check(original)
    except AssertionError:
        pass
    else:
        raise AssertionError("check accepts the unpatched source")
    assert fails(original.replace(repair.ANCHOR, ""), work), "missing anchor accepted"
    assert fails(original.replace(repair.ANCHOR, repair.ANCHOR * 2), work), "repeated anchor accepted"
    assert fails(original.replace(repair.NAMESPACE, "\n"), work), "missing namespace accepted"
print("headless-clap-host: ok")
