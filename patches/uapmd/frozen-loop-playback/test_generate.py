"""Validate the real upstream input and fail closed on changed assumptions."""
from pathlib import Path
import unittest
from generate import POSITION, READ, transform

SOURCE = Path(__file__).resolve().parents[4] / 'uapmd/source/uapmd-engine/src/sequencer/FrozenTrackManager.cpp'


class GenerateTests(unittest.TestCase):
    def test_real_upstream(self):
        patched = transform(SOURCE.read_text(encoding='utf-8'))
        self.assertIn('!engine.offlineRendering() && loop.loopEnabled', patched)
        self.assertIn('repl_frozen_loop::wrap(', patched)
        self.assertIn('repl_frozen_loop::read(', patched)
        self.assertIn('output + offset, frames, waitForDisk', patched)

    def test_changed_or_repeated_application_fails(self):
        source = SOURCE.read_text(encoding='utf-8')
        for changed in (source.replace(POSITION, ''), source + READ, transform(source)):
            with self.assertRaises(ValueError):
                transform(changed)


if __name__ == '__main__':
    unittest.main()
