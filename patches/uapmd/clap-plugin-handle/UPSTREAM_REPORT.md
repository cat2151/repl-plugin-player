# The CLAP plugin pointer of an instance is reachable only with ARA

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告; draft only.

Some CLAP plugins offer their own extensions (for example a preset loader
outside `clap.preset-load`). A host application that wants to call one needs the
instance's `clap_plugin*`. In UAPMD this is returned by
`AraHandleExtension::nativeHandle(AraPluginInstanceHandleKind::CLAPPlugin)`
(`source/remidy/src/clap/PluginFormatCLAP.hpp`), which is compiled only under
`UAPMD_HAS_ARA`. With `UAPMD_ENABLE_ARA=OFF`, `PluginInstanceCLAP::plugin` is
private with no accessor, and `RemidyAudioPluginInstance` (private to
uapmd-plugin-hosting) is the only way from `AudioPluginInstanceAPI` to the remidy
instance.

Suggestion: expose the native handle independently of ARA, for example an
instance extension available in every build that returns the format's native
pointer (`clap_plugin*`, VST3 component, ...), with ARA reusing it.

The app works around this by reading the private member through an explicit
template instantiation in an added translation unit, which depends on the
member's name and type.
