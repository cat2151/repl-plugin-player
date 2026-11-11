"""Connect prepared MIDI to upstream's existing render segments, without checkout edits."""
from pathlib import Path
import subprocess
import sys


def transform(source, header):
    edits = [
        ('        auto snapshot = timeline_tracks_snapshot_.protect();',
         '        if (repl_loop::instance) repl_loop::instance->begin(engine_, timeline_, process.frameCount());\n'
         '        auto snapshot = timeline_tracks_snapshot_.protect();'),
        ('        auto wrapToLoopRange = [this](int64_t samplePosition) -> int64_t {',
         '        auto wrapToLoopRange = [this](int64_t samplePosition) -> int64_t {\n'
         '            if (repl_loop::instance && repl_loop::instance->enabled())\n'
         '                return repl_loop::instance->position(samplePosition);'),
        ('                auto segmentTimeline = renderTransport;',
         '                const auto segmentRaw = renderBaseSample + static_cast<int64_t>(trackOffset) + destinationOffsetFrames;\n'
         '                const bool managedLoop = repl_loop::instance && repl_loop::instance->enabled();\n'
         '                if (managedLoop) segmentStartSample = repl_loop::instance->position(segmentRaw);\n'
         '                auto segmentTimeline = renderTransport;'),
        ('                segmentPosition.samples = wrapToLoopRange(segmentStartSample);',
         '                segmentPosition.samples = segmentStartSample;'),
        ('                if (timeline_.loopEnabled &&',
         '                if (managedLoop) {\n'
         '                    segmentFrames = static_cast<int32_t>(std::min<int64_t>(remainingFrames, repl_loop::instance->span(segmentRaw)));\n'
         '                    segmentTimeline.loopStart.samples = 0;\n'
         '                    segmentTimeline.loopEnd.samples = repl_loop::instance->timing(segmentRaw).first;\n'
         '                } else if (timeline_.loopEnabled &&'),
        ('                destinationOffsetFrames += segmentFrames;',
         '                if (repl_loop::instance) repl_loop::instance->segment(\n'
         '                    *(*snapshot)[i], i, trackContext->eventIn(),\n'
         '                    renderBaseSample + static_cast<int64_t>(trackOffset) + destinationOffsetFrames,\n'
         '                    segmentStartSample, segmentFrames, destinationOffsetFrames, segmentTimeline.isPlaying);\n\n'
         '                destinationOffsetFrames += segmentFrames;'),
    ]
    for anchor, replacement in edits:
        if source.count(anchor) != 1:
            raise ValueError(f'expected unique anchor: {anchor.strip()}')
        source = source.replace(anchor, replacement)
    return f'#include "{header.as_posix()}"\n' + source


def main():
    upstream, output, header = map(Path, sys.argv[1:])
    source = upstream / 'source/uapmd-engine/src/sequencer/TimelineFacadeAudio.cpp'
    revision = subprocess.check_output(['git', '-C', str(upstream), 'rev-parse', 'HEAD'], text=True).strip()
    try:
        result = transform(source.read_text(encoding='utf-8'), header)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(result, encoding='utf-8')
    except (OSError, ValueError) as error:
        sys.exit(f'loop-playback: {source}: {error}; upstream {revision}; see {Path(__file__).with_name("README.md")}')


if __name__ == '__main__':
    main()
