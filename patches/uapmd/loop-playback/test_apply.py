"""Regression contract for the generated upstream connection (no build mutations)."""
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
from apply import transform

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT.parent / 'uapmd/source/uapmd-engine/src/sequencer/TimelineFacadeAudio.cpp'


class ConnectionTests(unittest.TestCase):
    def test_preserves_upstream_and_uses_existing_segments(self):
        original = SOURCE.read_text(encoding='utf-8')
        result = transform(original, Path('X:/app/native/loop_playback.hpp'))
        self.assertEqual(result.count('repl_loop::instance->begin('), 1)
        self.assertEqual(result.count('repl_loop::instance->segment('), 1)
        self.assertIn('renderBaseSample + static_cast<int64_t>(trackOffset) + destinationOffsetFrames', result)
        self.assertEqual(result.count('processAudioForRenderSegment('), original.count('processAudioForRenderSegment('))
        self.assertIn('((samplePosition - timeline_.loopStart.samples) % loopLength)', result)
        self.assertIn('repl_loop::instance->position(samplePosition)', result)
        self.assertIn('repl_loop::instance->span(segmentRaw)', result)
        self.assertIn('segmentTimeline.loopEnd.samples = repl_loop::instance->timing(segmentRaw).first', result)
        self.assertIn('segmentPosition.samples = segmentStartSample;', result)
        self.assertEqual(SOURCE.read_text(encoding='utf-8'), original)

    def test_changed_or_duplicate_anchor_fails_closed(self):
        original = SOURCE.read_text(encoding='utf-8')
        anchors = [
            '        auto snapshot = timeline_tracks_snapshot_.protect();',
            '        auto wrapToLoopRange = [this](int64_t samplePosition) -> int64_t {',
            '                auto segmentTimeline = renderTransport;',
            '                segmentPosition.samples = wrapToLoopRange(segmentStartSample);',
            '                if (timeline_.loopEnabled &&',
            '                destinationOffsetFrames += segmentFrames;',
        ]
        for anchor in anchors:
            for source in (original.replace(anchor, ''), original + '\n' + anchor):
                with self.subTest(anchor=anchor, duplicate=source.endswith(anchor)):
                    with self.assertRaisesRegex(ValueError, 'expected unique anchor'):
                        transform(source, Path('X:/app/native/loop_playback.hpp'))


if __name__ == '__main__':
    unittest.main()
