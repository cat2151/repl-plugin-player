"""Check real source anchors and reject changed or already patched inputs."""
from pathlib import Path
import unittest
from generate import audio, header, manager

ROOT = Path(__file__).resolve().parents[3]
UPSTREAM = ROOT.parent / 'uapmd/source/uapmd-engine/src/sequencer'


class GenerateTests(unittest.TestCase):
    def test_source_guards(self):
        for name, transform in [('FrozenTrackAudio.cpp', audio), ('FrozenTrackAudio.hpp', header)]:
            source = (UPSTREAM / name).read_text(encoding='utf-8')
            patched = transform(source)
            self.assertNotEqual(source, patched)
            with self.assertRaises(ValueError):
                transform(patched)
            with self.assertRaises(ValueError):
                transform('')

    def test_manager_guards(self):
        source = (ROOT / 'target/shim/freeze-monitor/FrozenTrackManager.cpp').read_text(encoding='utf-8')
        patched = manager(source, ROOT / 'target/shim/frozen-loop-prefetch')
        self.assertIn('looping && loop.loopEnd.samples > loop.loopStart.samples', patched)
        with self.assertRaises(ValueError):
            manager(patched, ROOT)


if __name__ == '__main__':
    unittest.main()
