"""Keep routed builtins in the DAG, with only the implicit track fader after it."""
import hashlib
import re
from pathlib import Path
import subprocess
import sys

BASELINE = 'feaeb96d341328acaa33dac9f8ef9bcbe1cd3201b282b9e73d83cdf6cf420ff4'
HERE = Path(__file__).resolve().parent


def generate(source, output, node_header=None, engine_source=None):
    source, output = Path(source).resolve(), Path(output).resolve()
    if source == output or source.parent in output.parents:
        raise ValueError('output must be outside the dependency source directory')
    text = source.read_bytes().replace(b'\r\n', b'\n')
    if hashlib.sha256(text).hexdigest() != BASELINE:
        raise ValueError('baseline SHA256 mismatch; inspect upstream change/fix')
    text = text.decode('utf-8')

    def once(old, new):
        nonlocal text
        if text.count(old) != 1:
            raise ValueError(f'expected one occurrence: {old!r}')
        text = text.replace(old, new)

    for comment in [
        '        // Built-in nodes are processed by the dedicated post-DAG chain below.\n'
        '        // Keep the default linear topology limited to plug-in nodes so a built-in\n'
        '        // such as the implicit track gain is not compiled and processed twice.\n',
        '        // Built-in nodes are applied after the plug-in DAG in processAudio().\n'
        '        // They must not become DAG runtimes as well, or they would be processed\n'
        '        // once in topo_order and once again by the post-DAG chain.\n',
    ]:
        once(comment + '        auto& orderedNodes = state.nodes;',
             '        std::vector<std::shared_ptr<AudioGraphNode>> orderedNodes(state.nodes.begin(), state.nodes.end());\n'
             '        for (const auto& node : state.builtin_nodes)\n'
             '            if (node && node->nodeId() != "builtin:track_gain")\n'
             '                orderedNodes.push_back(node);')
    once('''        // Built-in nodes are outside the plug-in DAG and form the final serial
        // stage, matching the ordering used by AudioPluginGraph.
        if (!state.builtin_nodes.empty()) {
            if (!state.nodes.empty())
                process.advanceToNextNode();
            for (size_t i = 0; i < state.builtin_nodes.size(); ++i) {
                if (state.builtin_nodes[i])
                    state.builtin_nodes[i]->processAudio(process);
                if (i + 1 < state.builtin_nodes.size())
                    process.advanceToNextNode();
            }
        }''', '''        // The implicit track fader stays after all routed audio.
        for (const auto& node : state.builtin_nodes) {
            if (node && node->nodeId() == "builtin:track_gain") {
                process.advanceToNextNode();
                node->processAudio(process);
            }
        }''')
    header = Path(node_header).resolve() if node_header else source.parent / 'AudioPluginNodeImpl.hpp'
    if not header.is_file():
        raise ValueError('node header missing')
    once('#include "AudioPluginNodeImpl.hpp"', f'#include "{header.as_posix()}"')
    text = f'#include "{(HERE / "routing.hpp").as_posix()}"\n' + text
    pattern = (r'        bool appendEventsForGroup\(remidy::EventSequence& dst, '
               r'remidy::EventSequence& src, uint8_t group\) \{\n.*?\n        \}')
    text, count = re.subn(pattern, '''        bool appendEventsForGroup(remidy::EventSequence& dst, remidy::EventSequence& src, uint8_t group) {
            return repl_dag::appendEventsForGroup(dst, src, group);
        }''', text, flags=re.S)
    if count != 1:
        raise ValueError('expected one group event filter')
    once('            if (pluginImpl && pluginNode && instance) {',
         '            if (pluginImpl && pluginNode && instance) {\n'
         '                repl_dag::normalizeInstanceGroup(runtime.process.eventIn(), group);')
    once('pluginImpl->prepareEventInput(runtime.process.eventIn(), group);',
         'pluginImpl->prepareEventInput(runtime.process.eventIn(), 0xFF);')
    outputs = {output: text.encode('utf-8')}
    if engine_source is not None:
        engine_source = Path(engine_source).resolve()
        raw = engine_source.read_bytes().replace(b'\r\n', b'\n')
        if hashlib.sha256(raw).hexdigest() != '93821bdd4da436c684804f3c29ba57dee32c70f358a43b0ebe18a8aaf1fa2adc':
            raise ValueError(f'{engine_source}: baseline SHA256 mismatch; inspect upstream change/fix')
        engine = raw.decode('utf-8')
        old = 'track.setProcessingGroup(instanceId, fb ? fb->group() : static_cast<uint8_t>(0xFF));'
        if engine.count(old) != 1:
            raise ValueError(f'{engine_source}: expected unique processing group assignment')
        engine = engine.replace(old, '''const auto assignment = explicit_group_assignments_.find(instanceId);
            const auto assigned = assignment == explicit_group_assignments_.end() ? static_cast<uint8_t>(0xFF) : assignment->second;
            track.setProcessingGroup(instanceId, repl_dag::processingGroup(assigned, fb ? fb->group() : static_cast<uint8_t>(0xFF)));''')
        fields = '        UapmdFunctionBlockManager function_block_manager{};'
        setter = '                    t->setInstanceGroup(instanceId, group);'
        erase = 'plugin_instances_.erase(instanceId);'
        if engine.count(fields) != 1 or engine.count(setter) != 1 or engine.count(erase) != 2:
            raise ValueError(f'{engine_source}: explicit group lifecycle anchors changed')
        engine = engine.replace(fields, fields + '\n        std::unordered_map<int32_t, uint8_t> explicit_group_assignments_;')
        engine = engine.replace(setter, setter + '\n                    explicit_group_assignments_[instanceId] = group;')
        engine = engine.replace(erase, erase + '\n            explicit_group_assignments_.erase(instanceId);')
        engine = re.sub(r'#include "([^"\n]+)"',
            lambda m: f'#include "{(engine_source.parent / m[1]).resolve().as_posix()}"'
                if (engine_source.parent / m[1]).is_file() else m[0], engine)
        engine = f'#include "{(HERE / "routing.hpp").as_posix()}"\n#include <unordered_map>\n' + engine
        outputs[output.parent / engine_source.name] = engine.encode('utf-8')
    output.parent.mkdir(parents=True, exist_ok=True)
    for path, encoded in outputs.items():
        if not path.exists() or path.read_bytes() != encoded:
            path.write_bytes(encoded)


if __name__ == '__main__':
    try:
        generate(*sys.argv[1:])
    except Exception as error:
        source = Path(sys.argv[1])
        revision = subprocess.run(['git', '-C', str(source.parent), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'dag-builtins: {source}: {error}; upstream {revision}; see {HERE / "README.md"}')
