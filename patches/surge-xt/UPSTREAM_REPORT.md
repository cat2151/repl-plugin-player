# Notes received shortly after All Sound Off (CC120) are silenced

Status: draft, not sent.

## Summary
Surge XT (CLAP, 1.3.4 on Windows) drops a note-on that arrives within 256 samples
after an All Sound Off (CC 120). The note is accepted, but it never becomes audible,
so a host that sends "All Sound Off, then restart from the top" in one process call
loses the first note of the restart.

## Steps to reproduce
1. Host at 48 kHz, 1024-frame process calls.
2. In one process call, send CC 120 (and CC 123) on all channels at time 0, then a
   note-on at time `D` in the same call, holding the note for 0.5 s.
3. Observe the output peak for the next 400 ms.

Measured result (three repetitions per `D`, the count is the number of 30 ms polls
with a zero output peak):

| `D` (samples) | 0 | 100 | 200 | 250 | 300 | 400 | 600 | 800 | 1500 |
|---|---|---|---|---|---|---|---|---|---|
| silent polls | 12-14 | 11-14 | 12-13 | 12-14 | 0 | 0 | 0 | 0 | 0 |

The threshold lies between 250 and 300 samples, i.e. 8 internal blocks of 32 samples,
and does not depend on whether the note is in the same host process call (all `D`
above are in the same call). Another CLAP synth (Six Sines) plays the note at `D = 0`.

## Cause
`SurgeSynthesizer::allSoundOff()` sets `approachingAllSoundOff = true` and
`masterfade = 1.f`. `process()` then decrements `masterfade` by `0.125f` per block
("kill over 8 blocks") and, when it reaches zero, releases both scenes
(`releaseScene(0)`, `releaseScene(1)`; `stopSound()` on current main). Voices started
after the CC 120 but before the fade ends are released together with the old ones,
and they are also attenuated by `amp_mute` during the fade.

## Expected
Only voices that existed when CC 120 was received are silenced; a note-on received
after CC 120 starts a voice normally.

## Possible fix
Mark the voices alive at the time of `allSoundOff()` and release only those at the end
of the fade, and do not apply the fade to voices started afterwards.
