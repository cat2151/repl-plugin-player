# Frozen playback bypasses input/output observations

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告; draft only.

The app observes delivered UMP and plugin output in AudioPluginNodeImpl. After
enabling track freeze, FrozenTrackManager bypasses those nodes, so both the
keyboard and oscilloscope lose their input during audible cached playback.

The local repair observes scheduled UMP and rendered track output in the frozen
processor without running plugin DSP. A public track-observation callback could
support this directly. Intermediate plugin outputs are unavailable in a track
cache, so the frozen scope represents the rendered track output.

Separately, playback remains active beyond content end. A freeze requested there
stays deferred until explicit Stop. The app now pauses a finite, non-looping score
at content end when freezing is pending, then lets the regular tail/quiet path
start rendering. Active device input is excluded from this automatic pause.

Regression: a held MIDI note must light the keyboard and produce a nonzero pitched
waveform while frozen audio plays. Requesting freeze during finite playback must
eventually produce a frozen state without calling Stop.
