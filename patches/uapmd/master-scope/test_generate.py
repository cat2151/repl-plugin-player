from pathlib import Path
import unittest
from generate import ANCHOR, transform


class MasterScopeTest(unittest.TestCase):
    def test_observes_once_after_processing_and_only_on_success(self):
        result = transform(ANCHOR, Path('X:/repo/native/scope_capture.hpp'))
        self.assertIn('const auto status = sequencer->processAudio(process);\n'
                      '        if (status == 0) repl_capture_scope(-2, process);\n'
                      '        return status;', result)
        self.assertEqual(result.count('repl_capture_scope('), 1)

    def test_changed_or_duplicate_anchor_fails_closed(self):
        for text in ['', ANCHOR * 2]:
            with self.assertRaises(ValueError):
                transform(text, Path('X:/repo/native/scope_capture.hpp'))


if __name__ == '__main__':
    unittest.main()
