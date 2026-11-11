# Draft upstream report (not sent)

Revision: a4a96938eb31ddcb70209d62caf0dcc6ea9d3abe, Windows, Surge XT CLAP.

Calling `uapmd.instance(id).getPresets()` on a prepared Surge XT fails with
`No mapping for the Unicode character exists in the target multi-byte code page.`
The installed factory collection contains filenames outside the Windows ANSI
code page. CLAPHelper converts directory entries using `path.string()` although
CLAP expects UTF-8. Use UTF-8 filesystem input/output conversions.

CLAPPresetInfo also needs owned load-key and location data. The current load
implementation always passes LOCATION_PLUGIN/null even for file presets. Pass
LOCATION_FILE, the UTF-8 file path, and the provider's key (nullable for an ordinary
single-preset file). Copy callback-owned strings before get_metadata returns;
accept a null optional name using the file stem as fallback. Capture this/index
by value when scheduling synchronous loads, and validate both overloads.

Discovery failures must release providers. Native callback exceptions must be
caught inside the QuickJS callback: unwinding through choc invokeCallback has
left subsequent script evaluations unstable. Return a failed MutationJob for
load/discovery failure; report enumeration errors through a safe host boundary.

Reproduction and real-plugin checks are in the app's clap-presets regression.
The local solution is an app-owned generated overlay; it does not edit the
upstream checkout. No report has been transmitted.

Additional installed-Surge findings: the provider rejects UTF-8 non-ASCII paths
through its narrow filesystem/stream boundary. Existing, losslessly encodable
Windows short names let it read those files. It also intermittently reports XML
parse failures for independently valid factory FXPs. The app uses a narrowly
scoped single-FXP filename fallback for the exact Surge provider ID; original
files remain unchanged. Invalid UTF-8 provider names use the original file stem.

`MutationJob::resultJson` starts as `null`, but `mutationJobValue` passes it to
`choc::json::parse` (objects/arrays only). `loadPreset` therefore throws before
returning its running job. Use `parseValue` for nullable pending results.

## Surge 1.3.4 after processing has stopped

The installed descriptor reports 1.3.4. Verified source: Surge commit
f7b97c682ade0b87da85ca5968b63d5c7c98e68d (release_xt_1.3.4), with
clap-juce-extensions 087dd16e2d17e6b3f1c9a92817a8a7348ea08168.
Surge retains audio_processing_active after CLAP reset/releaseResources. A later
file load is queued but not applied until more processing; from_location returns
true and the wrapper sends preset-loaded immediately. The application's normal
silent Ready check therefore exposed completed jobs with unchanged Init Saw state.

The local stopped-instance workaround processes only that Surge into discarded
buffers and waits for a new host stateMarkDirty generation. In the verified source
this is emitted via processBlockPostFunction only after the background loader's
patchChanged flag; prior throttled notifications are drained before submission.
It does not enable the device/engine, advance other nodes or recreate the plugin.
Missing completion within ten seconds fails the job; other descriptor versions
require revalidation. Parameter rescan, preset-loaded callback, matching name and
fixed sleep/block counts do not independently prove completion.

The CLI regression uses normal Ready and checks actual factory parameters,
edited and unchanged same-preset reloads, Unicode loads, failure recovery and
unchanged other-instrument/effect states. An upstream correction should handle
stopped loading and report completion after applying the patch, including when
the resulting values are identical. No dependency checkout or installed plugin
is modified, and this report remains an unsent draft.
