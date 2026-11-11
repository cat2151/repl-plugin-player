# CLAP preset discovery and file loading

## Cause and scope

Verified upstream: `a4a96938eb31ddcb70209d62caf0dcc6ea9d3abe`.
On Windows, `CLAPHelper.hpp` converts a discovered wide filesystem path with
`path.string()`. A factory filename outside the active ANSI code page throws
before enumeration finishes. CLAP paths are UTF-8. The loader also loses the
file location and retains a borrowed load-key pointer; both load variants then
incorrectly request a plugin-internal preset with a null location.

The app-owned overlay uses UTF-8 paths in both directions and owns each preset's
location kind, file path and load key. Optional or invalid UTF-8 names fall back to the UTF-8 file stem.
The installed Surge provider also interprets paths through Windows ANSI streams:
if UTF-8 discovery fails, retry an existing short alias encoded losslessly in the
active code page. The alias is used only for discovery. Loading retains the canonical UTF-8
location: Surge uses a separate UTF-8-aware load path. No alias
is created and no preset is renamed or copied. If no suitable alias exists, an
unrecoverable provider error is reported rather than dropping a Unicode filename.

The installed provider intermittently rejects valid FXP XML (direct provider
probes reproduced this while independent bounded XML parsing succeeded). For its
exact provider ID only, a failed metadata scan of a verified single-preset Surge
FXP falls back to the original UTF-8 filename, FILE location and empty key. The
validator checks CcnK/FPCh/cjs3/sub3 headers and bounded XML length. This recovers
discoverable files without parsing or rewriting legacy XML. The filename may
differ from the patch's internal display name. Other providers retain normal
error reporting. A file literally named `.fxp` has no filesystem extension and
is not listed by extension matching. Provider initialization is separated from allocation so failures destroy
already-created providers. Provider lists are reset for each provider. All CLAP
translation units use the same generated headers, avoiding different type layouts.

The runtime overlay composes with `script-api`: enumeration exceptions are caught
inside the native QuickJS callback and reported by the host after evaluation.
The running MutationJob's null result uses `choc::json::parseValue`, because
`parse` accepts only objects/arrays and used to throw during load submission.
Loading validates discovery/index before scheduling and returns failed MutationJobs
on errors. Asynchronous load exceptions complete the job instead of escaping the
control-thread task. Existing GUI exception handling is preserved.

App-owned diagnostics call the bridge's `repl_localized` catalog adapter. It uses
`script-api`'s synchronous Rust host callback on the control thread and copies
the returned UTF-8 string before another callback. Plugin/provider diagnostics,
preset names and paths remain unchanged. No translation callback enters JS or
the audio thread. The standalone discovery fixture supplies a key-only adapter;
`scripts/native_text.py` supplies the real catalogs to DLL-only UI regressions.

### Stopped Surge 1.3.4 load completion

Normal Ready briefly processes the silent engine and stops it. Installed Surge
XT CLAP 1.3.4 retains `audio_processing_active=true` after processing stops, so
its file loader queues a patch that will not apply until processing resumes.
Completing the MutationJob at `from_location` return leaves the real state and
editor on Init Saw. The wrapper's preset-loaded callback also reports acceptance
immediately, and parameter rescans alone do not prove actual completion.

The verified source is Surge release_xt_1.3.4, commit
[`f7b97c682ade0b87da85ca5968b63d5c7c98e68d`](https://github.com/surge-synthesizer/surge/tree/f7b97c682ade0b87da85ca5968b63d5c7c98e68d),
with clap-juce-extensions submodule
[`087dd16e2d17e6b3f1c9a92817a8a7348ea08168`](https://github.com/free-audio/clap-juce-extensions/tree/087dd16e2d17e6b3f1c9a92817a8a7348ea08168).
In that processor the background loader sets `patchChanged` after loading.
`processBlockPostFunction` exchanges it and emits non-parameter state change;
the wrapper forwards that to host `stateMarkDirty`. This is its only
non-parameter-state-change/mark-dirty path. Unlike name/value matching, it also
identifies completion of an unchanged same-preset reload.

`stopped-surge.hpp` applies only to the exact Surge XT CLAP ID, version 1.3.4,
FILE presets and a fully stopped configured 32-bit instance. It excludes
concurrent processing and processes this instance into owned zero-input/discarded
output buffers, with no input events and stopped transport. It does not start
the engine/device, process other instruments/effects/clips, or replace an instance.
Twelve process calls before submission exhaust 1.3.4's
`checkNamesEvery++ > 10` throttle, and each is followed by the wrapper's
main-thread callback drain. Only then is the dirty generation captured and the
preset submitted. Completion requires a new dirty generation; the twelve calls
are stale-notification drainage, not a fixed-block success test.
Ten seconds without actual change notification or a processing error fails the
job. Other Surge versions fail with a diagnostic requiring revalidation; active
instances and other plugins retain their existing path. Startup Init Saw
verification is unchanged.

## Application and failure investigation

`apply.cmake` runs `generate.py` after `script-api`, generates sources only under
`target/shim/clap-presets`, and replaces the remidy target's CLAP sources. The
runtime remains at `target/shim/script-api/UapmdJSRuntime.cpp`. Dependency checkouts
are never modified. Existing cache and fresh clones use the same generation path;
upstream input changes and generator changes trigger CMake reconfiguration.

Expected transformation anchors are checked exactly. On failure, configuration
stops with patch name, source directory, upstream revision and this document;
the Python diagnostic identifies the missing anchor. Compare the changed upstream
source with `generate.py` and check whether upstream already fixed the behavior.
Do not bypass anchor checks or compile a partially generated overlay.

Run `python scripts/build.py`, then `python scripts/cli-test.py` and
`python scripts/plugin-ui-test.py`. The default CLI check covers normal Ready,
actual FXP parameter values, edited and unchanged same-preset reloads, three
Unicode patches, failed jobs and recovery, graph/clip preservation and byte-equal
Six Sines/effect states. The CLI check's `--core` mode intentionally excludes
preset acceptance. Factory reference decoding explicitly handles the legacy
Windows-1252 name in the original micro-computer FXP; saved plugin state remains
strict UTF-8. For installed-list presets Surge 1.3.4 `loadPatch` supplies the
filesystem-derived patch name to `loadPatchByPath`, so Sunrise's expected display
name uses its original filename stem rather than the differently capitalized XML
name; no original numeric parameters are omitted from comparison.
The expected-values helper also applies 1.3.4's documented BBD Ensemble
`handleStreamingMismatches` rule: effect type 20, streaming revision <= 22,
parameter 11 becomes the deactivated output filter's `defaultFilterCut`
(45.2326448f, 6 kHz), replacing an old dummy integer zero. This is derived from
the pinned upstream effect implementation, not from the application under test.
The DLL regression uses installed Surge in a child
process, reads the actual factory data and writes states/diagnostics only to an
external temporary directory. Preset checks do not start device playback or edit
original presets. The CLI uses the normal silent Ready preparation.

## Removal and reporting

Remove the overlay, its include in native/CMakeLists.txt, its native error handoff, and
its AGENTS.md entry once upstream preserves UTF-8 paths, owned discovery locations
and keys, correct file loads and safe script exception boundaries. Keep the
behavioral regression or adapt it to the upstream implementation before removal.
Remove the stopped-Surge helper, its generated method/counter and host dirty
callback when the installed plugin handles stopped loads and reports actual
completion reliably. Revalidate initial, edited-reload and unchanged-reload
acceptance before removal or supporting another descriptor version. The helper
is included by the generated source; normal dependency tracking compiles changes
in existing caches, while fresh clones use the same generation path. Upstream
anchor changes still fail configuration rather than silently omitting a fix.

Author report status: **not sent**. See `upstream-report.md`; drafting does not
authorize external transmission. New-cache reproducibility is not claimed by
an existing-cache build.
