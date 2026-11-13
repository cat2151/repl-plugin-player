"""Copy the upstream CLAP parameter source into the shim, rejecting changed upstream shapes."""
from pathlib import Path
import sys


def generate(source, directory):
    path = directory / source.name
    text = source.read_text(encoding='utf-8')
    changes = [
        ('#include "PluginFormatCLAP.hpp"',
         '#include "' + (source.parent / 'PluginFormatCLAP.hpp').resolve().as_posix() + '"\n#include "' +
         (Path(__file__).parent / 'metadata.hpp').resolve().as_posix() + '"'),
        ('        clearParameterList();\n    }',
         '        clearParameterList();\n        repl_parameters::destroy(this);\n    }'),
        ('    void PluginInstanceCLAP::ParameterSupport::clearParameterList() {',
         '    void PluginInstanceCLAP::ParameterSupport::clearParameterList() {\n        repl_parameters::invalidate(this);'),
        ('        return parameter_defs;\n    }',
         '        for (size_t i = 0; i < parameter_defs.size(); ++i) {\n'
         '            clap_param_info_t info{};\n'
         '            if (repl_parameters::requested == static_cast<int32_t>(parameter_defs[i]->index()) && owner->plugin && owner->plugin->paramsGetInfo(parameter_defs[i]->index(), &info) && info.id == parameter_ids[i])\n'
         '                repl_parameters::report(this, parameter_defs[i]->index(), info.id, info.flags);\n'
         '        }\n'
         '        return parameter_defs;\n    }'),
        ('    void PluginInstanceCLAP::ParameterSupport::refreshParameterMetadata(uint32_t index) {',
         '    void PluginInstanceCLAP::ParameterSupport::refreshParameterMetadata(uint32_t index) {\n        repl_parameters::invalidate(this);'),
    ]
    for old, new in changes:
        if text.count(old) != 1:
            raise RuntimeError(f'{source}: expected unique anchor: {old[:100]}')
        text = text.replace(old, new)
    directory.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


def runtime_metadata(runtime):
    text = runtime.read_text(encoding='utf-8')
    old = 'obj.setMember ("isReadonly", false); // Assuming all are writable for now'
    if text.count(old) != 1:
        raise RuntimeError(f'{runtime}: expected unique readonly placeholder')
    text = text.replace(old, 'obj.setMember ("isReadonly", repl_parameter_readonly(instanceId, static_cast<int32_t>(param.index)));\n            obj.setMember ("isDiscrete", param.discrete);')
    runtime.write_text('#include <cstdint>\nextern "C" bool repl_parameter_readonly(int32_t, int32_t);\n' + text, encoding='utf-8')


if __name__ == '__main__':
    generate(Path(sys.argv[1]), Path(sys.argv[2]))
    runtime_metadata(Path(sys.argv[3]))
