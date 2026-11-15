"""Check generation against the loop repair, including fail-closed anchors."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from generate import generate, transform

ROOT = Path(__file__).resolve().parents[3]
CHECKOUT = ROOT.parent / 'uapmd'
spec = importlib.util.spec_from_file_location('frozen_loop', Path(__file__).parents[1] / 'frozen-loop-playback/generate.py')
loop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loop)
SOURCE = CHECKOUT / 'source/uapmd-engine/src/sequencer/FrozenTrackManager.cpp'


class GenerateTests(unittest.TestCase):
    def test_real_source_and_headers(self):
        source = loop.transform(SOURCE.read_text(encoding='utf-8'))
        with tempfile.TemporaryDirectory(prefix='repl-freeze-monitor-') as temporary:
            directory = Path(temporary)
            input_path = directory / 'loop.cpp'
            input_path.write_text(source, encoding='utf-8')
            generate(CHECKOUT, input_path, directory)
            header = (directory / 'FrozenTrackManager.hpp').read_text(encoding='utf-8')
            self.assertIn('std::optional<repl_notes::InputState> repl_note_state;', header)
            patched = (directory / 'FrozenTrackManager.cpp').read_text(encoding='utf-8')
            self.assertIn('repl_capture_scope(target, context)', patched)
            self.assertIn('repl_frozen_loop::read(', patched)
            self.assertIn('if (state && !frozen) state->repl_note_state.reset();', patched)

    def test_changed_input_fails(self):
        source = loop.transform(SOURCE.read_text(encoding='utf-8'))
        for changed in (source.replace('    return state &&\n', ''), transform(source)):
            with self.assertRaises(ValueError):
                transform(changed)


if __name__ == '__main__':
    unittest.main()
