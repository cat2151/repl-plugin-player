"""Generate a checked frozen-playback repair without editing the checkout."""
from pathlib import Path
import subprocess
import sys

NAME = 'frozen-loop-playback'
REVISION = '9f708d3300b4a6f8109eee0fadccf286db8b7a45'
POSITION = '''    const int64_t cacheFrame =
        context.masterContext().playbackPositionSamples() -
        cachedAudio->startSample();'''
LOOP = '            loop = *loop_snapshot_.protect();'
INCLUDE = '#include "TimelineFacadeImpl.hpp"'
READ = '                    cachedAudio->read(cachedChannel, cacheFrame, output, frameCount, waitForDisk);'


def once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'expected one anchor, found {source.count(old)}: {old}')
    return source.replace(old, new)


def helper_path():
    return Path(__file__).with_name('segments.hpp').resolve().as_posix()


def transform_timeline(source):
    if 'repl_frozen_loop' in source:
        raise ValueError('repair already applied')
    source = once(source, INCLUDE, INCLUDE + f'\n#include "{helper_path()}"')
    return once(source, LOOP, '''        {
            loop = *loop_snapshot_.protect();
            repl_frozen_loop::publish(loop.enabled, loop.startSamples, loop.endSamples);
        }''')


def transform(source):
    helper = helper_path()
    source = once(source, '#include "FrozenTrackAudio.hpp"',
                  '#include "FrozenTrackAudio.hpp"\n' + f'#include "{helper}"')
    source = once(source, POSITION, '''    const auto position = context.masterContext().playbackPositionSamples();
    const auto loop = repl_frozen_loop::current();
    const bool looping = !engine.offlineRendering() && loop.enabled;
    const int64_t cacheFrame = repl_frozen_loop::wrap(
        position, looping, loop.start, loop.end) - cachedAudio->startSample();''')
    return once(source, READ, '''                    repl_frozen_loop::read(position, frameCount, looping,
                        loop.start, loop.end,
                        [&](int64_t source, int32_t offset, int32_t frames) {
                            cachedAudio->read(cachedChannel, source - cachedAudio->startSample(),
                                output + offset, frames, waitForDisk);
                        });''')


if __name__ == '__main__':
    source, output, timeline, checkout = map(Path, sys.argv[1:])
    try:
        files = {output: transform(source.read_text(encoding='utf-8'))}
        source = timeline
        files[output.with_name(timeline.name)] = transform_timeline(timeline.read_text(encoding='utf-8'))
        output.parent.mkdir(parents=True, exist_ok=True)
        for path, patched in files.items():
            if not path.exists() or path.read_text(encoding='utf-8') != patched:
                path.write_text(patched, encoding='utf-8')
    except (OSError, ValueError) as error:
        revision = subprocess.run(['git', '-C', str(checkout), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'{NAME}: {source}: {error}; upstream {revision} (validated {REVISION}); '
                 f'see {Path(__file__).with_name("README.md")}')
