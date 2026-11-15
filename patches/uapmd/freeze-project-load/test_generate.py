"""Validate the real source and reject changed or already applied anchors."""
from pathlib import Path
import unittest
from generate import ANCHOR, transform

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT.parent / 'uapmd/source/tools/uapmd-app-model/src/AppModel.cpp'


class GenerateTests(unittest.TestCase):
    def test_real_source(self):
        result = transform(SOURCE.read_text(encoding='utf-8'))
        start = result.index('void uapmd_app::AppModel::loadProject(')
        load = result[start:result.index('void uapmd_app::AppModel::newProject(', start)]
        self.assertLess(load.index('UapmdProjectDataReader::read'), load.index('freeze.unfreezeTrack'))
        self.assertLess(load.index('historyState().busy'), load.index('freeze.unfreezeTrack'))
        self.assertLess(load.index('engine.offlineRendering()'), load.index('\n    tearDownProjectInstances();'))
        self.assertIn('remidy::EventLoop::processQueuedTasks();', load)

    def test_changed_input_fails(self):
        source = SOURCE.read_text(encoding='utf-8')
        for changed in (source.replace(ANCHOR, ''), source + ANCHOR, transform(source)):
            with self.assertRaises(ValueError):
                transform(changed)


if __name__ == '__main__':
    unittest.main()
