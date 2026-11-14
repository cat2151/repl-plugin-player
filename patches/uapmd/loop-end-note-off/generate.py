"""Generate narrowly checked UAPMD copies; never modify the checkout."""
from pathlib import Path
import sys


def once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'expected one anchor, found {source.count(old)}: {old}')
    return source.replace(old, new)


def generate(root, output):
    header = (Path(__file__).parent / 'boundary.hpp').resolve().as_posix()
    prefix = f'#include "{header}"\n'
    track = (root / 'TimelineTrack.cpp').read_text(encoding='utf-8')
    track = once(track, '                midiNode->processEvents(', '''                // End-exclusive loop rendering would otherwise skip the final release.
                repl_loop_end::Scope loopEndRelease(
                    renderTimeline.isPlaying && renderTimeline.loopEnabled &&
                    renderTimeline.loopEnd.samples > renderTimeline.loopStart.samples &&
                    renderStartSample + frameCount == renderTimeline.loopEnd.samples &&
                    renderWindow->destinationOffsetFrames + renderWindow->processFrameCount == frameCount);
                midiNode->processEvents(''')
    midi = (root / 'MidiClipSourceNode.cpp').read_text(encoding='utf-8')
    midi = once(midi, '''            // Beyond current window
            if (eventSamples >= static_cast<uint64_t>(windowEnd))
                break;''', '''            // Only a loop's exact endpoint releases belong to the outgoing pass.
            // Keep all other events end-exclusive, including Note Ons at this time.
            const bool boundaryRelease = repl_loop_end::includeEndNoteOff && frameCount > 0 &&
                eventSamples == static_cast<uint64_t>(windowEnd);
            if (eventSamples >= static_cast<uint64_t>(windowEnd) && !boundaryRelease)
                break;''')
    midi = once(midi, '''            // Within window - emit or intercept the complete multi-word message
            if (eventSamples >= static_cast<uint64_t>(currentPos)) {''', '''            // Read only complete messages, including MIDI 2.0 Note On velocity zero.
            const bool release = repl_loop_end::isNoteOff(ump_events_[eventIdx],
                wordsNeeded >= 2 ? ump_events_[eventIdx + 1] : 0);
            if (boundaryRelease && !release) {
                eventIdx += wordsNeeded;
                continue;
            }
            // Clamp releases into this block; plugins must never receive offset == frameCount.
            if (boundaryRelease) eventSamples = static_cast<uint64_t>(windowEnd - 1);
            // Within window - emit or intercept the complete multi-word message
            if (eventSamples >= static_cast<uint64_t>(currentPos)) {''')
    output.mkdir(parents=True, exist_ok=True)
    for name, source in [('TimelineTrack.cpp', track), ('MidiClipSourceNode.cpp', midi)]:
        (output / name).write_text(prefix + source, encoding='utf-8')


if __name__ == '__main__':
    try:
        generate(Path(sys.argv[1]), Path(sys.argv[2]))
    except (OSError, ValueError) as error:
        sys.exit(str(error))
