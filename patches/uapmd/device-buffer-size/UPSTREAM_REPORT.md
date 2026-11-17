# MiniAudio ignores the requested buffer size, causing avoidable worker overruns

Tested revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`. Not yet submitted.

AppModel requests a 1024-frame buffer. MiniAudioIODeviceManager::onOpen discards
bufferSize, while MiniAudioIODevice initializes the device with a fixed 256-frame
period. On Windows at 48 kHz, four-track project playback repeatedly exceeds the
4.267 ms worker deadline and is silenced until late workers finish.

Please pass the requested buffer size into device initialization and use the same
size for AudioProcessContext allocation. Our local workaround uses the existing
preferred callback size member, retaining 256 for a zero request. Merely changing
the dispatcher's reported preferred size does not change the actual device period.

Honoring 1024 frames was verified on the real device, but is not a complete dropout
fix for this project: a sforzando + TONE3000 track still occasionally exceeds the
80% worker budget. A 2048-frame diagnostic also eventually exceeded that budget.
The local patch retains upstream's deadline policy and default requested size.

## Reconfigure starts a stopped device

`MiniAudioIODevice::reconfigure` always calls `start()` after reinitializing.
`RealtimeSequencer::reconfigureAudioDevice` restarts audio only when it was playing,
but with the audio engine disabled the device starts anyway. AppModel's
`setAudioEngineEnabled(false)` then cannot stop it, because `completeAudioEngineShutdown`
returns early once plugin processing is already stopped; `isAudioPlaying()` stays true.
Please restart the device in `reconfigure` only when it was running before. Our local
workaround does exactly that.
