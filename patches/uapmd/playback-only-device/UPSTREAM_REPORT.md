# Default duplex device underruns the render buffer after every start

Tested revision: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`. Not yet submitted.

`RealtimeSequencer` opens the audio device with `manager->open(-1, -1, ...)`, and
`MiniAudioIODeviceManager::onOpen` treats `-1` input as "default capture device", so the
device is `ma_device_type_duplex` even when nothing uses audio input.

MiniAudio's duplex loop (`ma_device_audio_thread__default_read_write`) reads one capture
period before calling the data callback and writing playback. On Windows WASAPI (48 kHz,
1024-frame period, 3072-frame render buffer) capture arrives every 20 or 30 ms while the
render endpoint consumes 480 frames every 10 ms. After `start()` the render padding before
each read grows only 64 frames per cycle (64, 128, ..., 448); about 140 ms in, a 30 ms
capture gap leaves 448 frames for a 480-frame read and Windows inserts 32 frames of silence.
Loopback recording shows an exact-zero run of 32 samples in every first note after a start.
The data callback's own output has no gap.

Opening the device playback-only (input index `kNoDeviceIndex`) removes the gap. Please
consider opening input only when an input device is explicitly requested, or decoupling
playback writes from capture arrival in the duplex path. Our local workaround never enables
input, because our application has no audio input.
