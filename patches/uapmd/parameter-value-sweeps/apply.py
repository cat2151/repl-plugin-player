"""Stop the two main-thread walks over every parameter value of a plugin instance.

Usage: apply.py <clap-note-dialect HostClasses.cpp> <UAPMD TimelineFacadePlugins.cpp> <output dir> <UAPMD checkout>
"""
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent

RESCAN_ANCHOR = """\
        if ((flags & CLAP_PARAM_RESCAN_VALUES) && !definitionsRebuilt) {
            // Parameter values changed - refresh all parameter values
            Logger::global()->logDiagnostic("CLAP: Parameter values changed, refreshing");
            auto& paramDefs = params->parameters();
            for (size_t i = 0; i < paramDefs.size(); ++i) {
                double value = 0.0;
                if (params->getParameter(static_cast<uint32_t>(i), &value) == StatusCode::OK) {
                    auto paramId = params->getParameterId(static_cast<uint32_t>(i));
                    params->notifyParameterValue(paramId, value);
                }
            }
        }
"""
RESCAN_REPLACEMENT = """\
        // parameter-value-sweeps: every value reader asks the plugin on demand, so a
        // value rescan needs no walk (see patches/uapmd/parameter-value-sweeps/README.md).
        (void) definitionsRebuilt;
"""

CACHE_ANCHOR = """\
    void TimelineFacadeImpl::refreshPluginParameterCache(int32_t instanceId) {
        auto* instance = engine_.getPluginInstance(instanceId);
        if (!instance)
            return;
        auto& values = plugin_parameter_values_[instanceId];
        for (const auto& parameter : instance->parameterMetadataList())
            values[static_cast<int32_t>(parameter.index)] =
                instance->getParameterValue(static_cast<int32_t>(parameter.index));
        // Some hosts expose parameter changes before (or without) a complete
        // metadata list. Refresh indices already observed by the timeline as
        // well so subsequent reads use the restored values.
        for (auto& [index, value] : values)
            value = instance->getParameterValue(index);
    }
"""
CACHE_REPLACEMENT = """\
    void TimelineFacadeImpl::refreshPluginParameterCache(int32_t instanceId) {
        // parameter-value-sweeps: nothing reads plugin_parameter_values_; drop the
        // instance's entries instead of asking the plugin for every value twice.
        plugin_parameter_values_.erase(instanceId);
    }
"""

REWRITES = ((RESCAN_ANCHOR, RESCAN_REPLACEMENT), (CACHE_ANCHOR, CACHE_REPLACEMENT))


def rewrite(source, output, anchor, replacement):
    source, output = Path(source).resolve(), Path(output).resolve()
    text = source.read_text(encoding="utf-8").replace("\r\n", "\n")
    found = text.count(anchor)
    if found != 1:
        first = anchor.splitlines()[0].strip()
        raise ValueError(f"{source}: expected exactly one anchor starting {first!r}, found {found}")
    text = text.replace(anchor, replacement)
    output.mkdir(parents=True, exist_ok=True)
    path = output / source.name
    if not path.exists() or path.read_text(encoding="utf-8") != text:
        path.write_text(text, encoding="utf-8", newline="\n")
    return path


def generate(host_classes, timeline_plugins, output):
    return [rewrite(source, output, anchor, replacement)
            for source, (anchor, replacement) in zip((host_classes, timeline_plugins), REWRITES)]


def main(argv):
    host_classes, timeline_plugins, output, uapmd = argv
    try:
        print("\n".join(path.as_posix() for path in generate(host_classes, timeline_plugins, output)))
    except (OSError, ValueError) as error:
        revision = subprocess.run(["git", "-C", uapmd, "rev-parse", "HEAD"],
                                  capture_output=True, text=True).stdout.strip() or "unknown"
        sys.exit(f"[parameter-value-sweeps] NOT applied; condition: {error}; "
                 f"UAPMD revision: {revision}; instructions: {HERE / 'README.md'}")


if __name__ == "__main__":
    main(sys.argv[1:])
