# Playback-only audio device

UAPMD `9f708d3300b4a6f8109eee0fadccf286db8b7a45` opens the default device as duplex
(capture + playback) unless the input index is `AudioIODeviceManager::kNoDeviceIndex`.
`RealtimeSequencer` and `uapmd.sequencer.reconfigureAudioDevice(-1, ...)` pass `-1`, so
the device is always duplex. MiniAudio's duplex loop writes playback only after one
capture period arrives. On Windows WASAPI with a 1024-frame period, capture arrives every
20 or 30 ms while the render side reads 480 frames every 10 ms, so the render padding grows
by only 64 frames per 20 ms after a start. About 140 ms after each device start a 30 ms gap
empties the buffer and Windows inserts 32 frames of silence: the first note of every
`play("cde")` (each play restarts the device) has an audible click-like gap.

This app has no audio input, so the generated source never enables the input direction.
Track input buses then receive silence instead of the default capture device. The device
callback, plugins and buffer sizes are unchanged.

`uapmd-host/CMakeLists.txt` includes `apply.cmake` after `device-buffer-size`. Configure
reads `target/shim/device-buffer-size/MiniAudioIODevice.cpp` and writes
`target/shim/playback-only-device/MiniAudioIODevice.cpp`, replacing the former in the
`uapmd-engine` sources; the checkout is never modified. Fresh checkouts and existing caches
follow the same path. A missing anchor, or the input not being in the target sources
(wrong include order or `device-buffer-size` removed), stops configuration with the patch,
file, condition, upstream revision and this document. Inspect upstream changes before
updating the anchor; do not bypass the check. If `device-buffer-size` is removed, read the
upstream file directly instead.

Run `python patches/uapmd/playback-only-device/test_generate.py` (it reads the sibling
`../uapmd` checkout) and `cargo build --release`. The dropout itself is only visible on
the real output device: `python scripts/first-note-dropout-test.py` loopback-records the
default device while `tests/e2e/first-note-dropout.js` restarts the engine and holds a
note three times, and fails on runs of exact zeros inside a note. Without this patch it
reports one 32-sample run about 137 ms into each note.

Remove the include and this directory when upstream opens playback only unless an input
device is requested, or its duplex path no longer underruns after start. Report status:
**未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
