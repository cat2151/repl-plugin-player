"""Check exact application and fail-closed behavior against the real upstream source."""
from pathlib import Path
import unittest
from generate import transform

SOURCE = (Path(__file__).resolve().parents[4] / "uapmd/source/uapmd-engine/src/devices/MiniAudioIODevice.cpp")


class GenerateTests(unittest.TestCase):
    def test_requested_period_reaches_device_and_buffer_allocation(self):
        patched = transform(SOURCE.read_text(encoding="utf-8"))
        self.assertIn("audio.setPreferredCallbackSize(bufferSize);", patched)
        self.assertIn("config.periodSizeInFrames = preferred_callback_frames_ > 0 ? preferred_callback_frames_ : 256;", patched)
        self.assertIn("data.configureMainBus(device->capture.channels, device->playback.channels, config.periodSizeInFrames);", patched)

    def test_upstream_change_or_double_application_fails(self):
        source = SOURCE.read_text(encoding="utf-8")
        for changed in (source.replace("    (void) bufferSize;", ""), source + "    (void) bufferSize;", transform(source)):
            with self.assertRaises(ValueError):
                transform(changed)


if __name__ == "__main__":
    unittest.main()
