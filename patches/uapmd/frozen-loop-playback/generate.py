"""Generate a checked frozen-playback repair without editing the checkout."""
from pathlib import Path
import subprocess
import sys

NAME = 'frozen-loop-playback'
REVISION = '4ec5c3b1c30130fdd3b7d4b375082273247e561f'
POSITION = '''    const int64_t cacheFrame =
        context.masterContext().playbackPositionSamples() -
        cachedAudio->startSample();'''
READ = '                    cachedAudio->read(cachedChannel, cacheFrame, output, frameCount, waitForDisk);'


def once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'expected one anchor, found {source.count(old)}: {old}')
    return source.replace(old, new)


def transform(source):
    helper = Path(__file__).with_name('segments.hpp').resolve().as_posix()
    source = once(source, '#include "FrozenTrackAudio.hpp"',
                  '#include "FrozenTrackAudio.hpp"\n' + f'#include "{helper}"')
    source = once(source, POSITION, '''    const auto position = context.masterContext().playbackPositionSamples();
    const auto& loop = engine.timeline().state();
    const bool looping = !engine.offlineRendering() && loop.loopEnabled;
    const int64_t cacheFrame = repl_frozen_loop::wrap(
        position, looping, loop.loopStart.samples, loop.loopEnd.samples) - cachedAudio->startSample();''')
    return once(source, READ, '''                    repl_frozen_loop::read(position, frameCount, looping,
                        loop.loopStart.samples, loop.loopEnd.samples,
                        [&](int64_t source, int32_t offset, int32_t frames) {
                            cachedAudio->read(cachedChannel, source - cachedAudio->startSample(),
                                output + offset, frames, waitForDisk);
                        });''')


if __name__ == '__main__':
    source, output, checkout = map(Path, sys.argv[1:])
    try:
        patched = transform(source.read_text(encoding='utf-8'))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding='utf-8') != patched:
            output.write_text(patched, encoding='utf-8')
    except (OSError, ValueError) as error:
        revision = subprocess.run(['git', '-C', str(checkout), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'{NAME}: {source}: {error}; upstream {revision} (validated {REVISION}); '
                 f'see {Path(__file__).with_name("README.md")}')
