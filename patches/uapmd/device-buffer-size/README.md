# Requested audio device buffer size

UAPMD `4ec5c3b1c30130fdd3b7d4b375082273247e561f` discards `bufferSize` in
`MiniAudioIODeviceManager::onOpen` and opens MiniAudio with a fixed 256-frame period.
AppModel requests 1024 frames, but parallel track rendering gets only an 80% deadline
of 256 frames (4.267 ms at 48 kHz). Overruns silence output until workers finish.

The generated source passes the requested size through the existing preferred-size
member before reconfigure and uses it both for the device period and context allocation.
A zero request retains the upstream 256-frame fallback. It does not change worker
deadlines, project data, or plugin state. Honoring 1024 instead of 256 frames increases
the nominal period from 5.33 to 21.33 ms at 48 kHz.

`native/CMakeLists.txt` includes `apply.cmake`. Configure regenerates only
`target/shim/device-buffer-size/MiniAudioIODevice.cpp`; the checkout is never modified.
Fresh checkouts and existing caches follow the same path. Source anchor mismatches or
missing CMake target entries stop configuration with the patch, file, condition,
upstream revision and this document. Inspect upstream changes before updating anchors;
do not bypass the check. Remove the include and this directory after verifying upstream
honors bufferSize through device initialization and context allocation.

Run `python patches/uapmd/device-buffer-size/test_generate.py` and the normal
`python scripts/build.py`. Then run
`python patches/uapmd/device-buffer-size/check_playback.py X:/music/song.uapmdz`
with a representative project. It starts three fresh CLI processes, plays 15 seconds
each, checks the inferred device cadence, finite nonzero output, worker faults and
callback deadline counters, and verifies the archive hash is unchanged. It uses the
real audio device and produces audible output; close other audio-heavy apps first.
Generator checks alone do not prove dropout elimination. Report status: **未報告**;
draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).

## Verification and remaining failure

The normal Windows release build and generator checks pass. Real-device playback
of an existing four-track archive confirms a 1024-frame cadence after this patch,
but the full playback check still fails: the sforzando + TONE3000 track sometimes
exceeds the 17.067 ms worker budget. The engine intentionally outputs silence at
that point. A diagnostic 2048-frame run passed twice and failed on its third run;
neither a larger default nor a different worker count is installed as a workaround.
This patch fixes the ignored device-size request, not all causes of audio dropouts.

The `loop` E2E passes. The `scope` E2E fails at the replaced instrument's note 64
waveform with both 256- and 1024-frame device periods; that failure remains unresolved.

The playback checker retains strict failure detection for the remaining problem.
`--frames` and `--workers` are temporary per-process diagnostic overrides; they do
not edit the archive or persist audio settings. The cadence is inferred from
callback counts and elapsed time, not a measurement of hardware output latency.
