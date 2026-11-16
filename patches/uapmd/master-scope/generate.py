"""Observe the final realtime master output without changing the checkout."""
from pathlib import Path
import sys

ANCHOR = '        return sequencer->processAudio(process);'


def transform(text, header):
    if text.count(ANCHOR) != 1:
        raise ValueError('expected one realtime processAudio return')
    return f'#include "{header.as_posix()}"\n' + text.replace(ANCHOR,
        '        const auto status = sequencer->processAudio(process);\n'
        '        if (status == 0) repl_capture_scope(-2, process);\n'
        '        return status;')


if __name__ == '__main__':
    source, output, header = map(Path, sys.argv[1:4])
    try:
        result = transform(source.read_text(encoding='utf-8'), header)
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding='utf-8') != result:
            output.write_text(result, encoding='utf-8')
    except (OSError, ValueError) as error:
        raise SystemExit(f'master-scope: {source}: {error}; upstream {sys.argv[4]}; '
                         f'see {Path(__file__).with_name("integration.md")}') from error
