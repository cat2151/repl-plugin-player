"""Give instances created inside repl_headless::Scope a HeadlessCLAPHost.

Usage: apply.py <clap-note-dialect PluginFormatCLAP.cpp> <output dir> <UAPMD checkout>
"""
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ANCHOR = "            auto host = std::make_unique<RemidyCLAPHost>();\n"
REPLACEMENT = ("            std::unique_ptr<RemidyCLAPHost> host = repl_headless::depth > 0\n"
               "                ? std::unique_ptr<RemidyCLAPHost>(std::make_unique<HeadlessCLAPHost>())\n"
               "                : std::make_unique<RemidyCLAPHost>();\n")
NAMESPACE = "\nnamespace remidy {\n"


def includes():
    return "".join(f'#include "{(HERE / name).as_posix()}"\n'
                   for name in ("headless_scope.hpp", "headless_host.hpp"))


def generate(source, output):
    source, output = Path(source).resolve(), Path(output).resolve()
    text = source.read_text(encoding="utf-8").replace("\r\n", "\n")
    for anchor in (ANCHOR, NAMESPACE):
        found = text.count(anchor)
        if found != 1:
            raise ValueError(f"expected exactly one {anchor!r}, found {found}")
    text = text.replace(ANCHOR, REPLACEMENT).replace(NAMESPACE, "\n" + includes() + NAMESPACE)
    output.mkdir(parents=True, exist_ok=True)
    path = output / source.name
    if not path.exists() or path.read_text(encoding="utf-8") != text:
        path.write_text(text, encoding="utf-8", newline="\n")
    return path


def main(argv):
    source, output, uapmd = argv
    try:
        print(generate(source, output).as_posix())
    except (OSError, ValueError) as error:
        revision = subprocess.run(["git", "-C", uapmd, "rev-parse", "HEAD"],
                                  capture_output=True, text=True).stdout.strip() or "unknown"
        sys.exit(f"[headless-clap-host] NOT applied; target: {source}; condition: {error}; "
                 f"UAPMD revision: {revision}; instructions: {HERE / 'README.md'}")


if __name__ == "__main__":
    main(sys.argv[1:])
