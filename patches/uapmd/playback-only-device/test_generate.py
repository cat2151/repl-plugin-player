"""Check the anchor against the real upstream source and fail-closed drift detection."""
from pathlib import Path
import unittest
from generate import OLD, transform

SOURCE = (Path(__file__).resolve().parents[4] / "uapmd/source/uapmd-engine/src/devices/MiniAudioIODevice.cpp")


class GenerateTests(unittest.TestCase):
    def test_input_direction_is_never_opened(self):
        patched = transform(SOURCE.read_text(encoding="utf-8"))
        self.assertIn("    const bool enableInput = false;", patched)
        self.assertNotIn(OLD, patched)
        self.assertIn("ma_device_config_init(enableInput ? ma_device_type_duplex : ma_device_type_playback)", patched)

    def test_upstream_change_or_double_application_fails(self):
        source = SOURCE.read_text(encoding="utf-8")
        for changed in (source.replace(OLD, ""), source + OLD, transform(source)):
            with self.assertRaises(ValueError):
                transform(changed)


if __name__ == "__main__":
    unittest.main()
