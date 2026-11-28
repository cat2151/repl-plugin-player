"""Generate a checked preroll repair without editing the checkout."""
from pathlib import Path
import subprocess
import sys

NAME = 'preroll-before-start'
REVISION = '277aa85468795267d6cf64953af587ea18c7f653'
# A render position before the song start stays negative; the clip windows then skip it.
CLAMP = '''            if (renderStartSample < 0)
                renderStartSample = 0;
'''


def transform(source):
    if source.count(CLAMP) != 1:
        raise ValueError(f'expected one anchor, found {source.count(CLAMP)}: {CLAMP.strip()}')
    return source.replace(CLAMP, '')


if __name__ == '__main__':
    source, output, checkout = map(Path, sys.argv[1:])
    try:
        content = transform(source.read_text(encoding='utf-8'))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding='utf-8') != content:
            output.write_text(content, encoding='utf-8')
    except (OSError, ValueError) as error:
        revision = subprocess.run(['git', '-C', str(checkout), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'{NAME}: {source}: {error}; upstream {revision} (validated {REVISION}); '
                 f'see {Path(__file__).with_name("README.md")}')
