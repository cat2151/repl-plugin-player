"""Generate a narrowly checked UAPMD copy; never modify the checkout."""
from pathlib import Path
import sys

ANCHOR = 'segmentStartSample + remainingFrames > loop.endSamples) {'


def transform(source):
    if source.count(ANCHOR) != 1:
        raise ValueError(f'expected one anchor, found {source.count(ANCHOR)}: {ANCHOR}')
    return source.replace(ANCHOR, ANCHOR.replace('>', '>='))


if __name__ == '__main__':
    try:
        source, output = map(Path, sys.argv[1:])
        content = transform(source.read_text(encoding='utf-8'))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding='utf-8') != content:
            output.write_text(content, encoding='utf-8')
    except (OSError, ValueError) as error:
        sys.exit(str(error))
