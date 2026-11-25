"""Validate the real upstream input and fail closed on changed assumptions."""
from pathlib import Path
import unittest
from generate import LOOP, POSITION, READ, transform, transform_timeline

SOURCE = Path(__file__).resolve().parents[4] / 'uapmd/source/uapmd-engine/src/sequencer/FrozenTrackManager.cpp'


class GenerateTests(unittest.TestCase):
    def test_real_upstream(self):
        patched = transform(SOURCE.read_text(encoding='utf-8'))
        self.assertIn('!engine.offlineRendering() && loop.enabled', patched)
        self.assertIn('repl_frozen_loop::wrap(', patched)
        self.assertIn('repl_frozen_loop::read(', patched)
        self.assertIn('output + offset, frames, waitForDisk', patched)

    def test_changed_or_repeated_application_fails(self):
        source = SOURCE.read_text(encoding='utf-8')
        for changed in (source.replace(POSITION, ''), source + READ, transform(source)):
            with self.assertRaises(ValueError):
                transform(changed)

    def test_timeline_hands_over_the_loop(self):
        source = SOURCE.with_name('TimelineFacadeAudio.cpp').read_text(encoding='utf-8')
        patched = transform_timeline(source)
        self.assertIn('repl_frozen_loop::publish(', patched)
        for changed in (source.replace(LOOP, ''), patched):
            with self.assertRaises(ValueError):
                transform_timeline(changed)


if __name__ == '__main__':
    unittest.main()
