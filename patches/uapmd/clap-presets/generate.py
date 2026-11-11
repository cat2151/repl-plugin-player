"""Generate app-owned CLAP preset fixes; reject unknown upstream shapes."""
from pathlib import Path
import re
import sys

upstream, output, runtime = map(Path, sys.argv[1:])
source = upstream / 'source/remidy/src/clap'
output.mkdir(parents=True, exist_ok=True)

def replace(text, old, new, count=1):
    if text.count(old) != count:
        raise RuntimeError(f'expected {count} occurrences: {old[:100]}')
    return text.replace(old, new)

texts = {p.name: p.read_text(encoding='utf-8') for p in source.iterdir() if p.suffix in ('.hpp', '.cpp')}
h = texts['CLAPHelper.hpp']
h = replace(h, 'const char *load_key;', 'std::string load_key;\n        uint32_t location_kind{};\n        std::string location;')
h = replace(h, 'std::vector<CLAPPresetInfo> presets{};', '''std::vector<CLAPPresetInfo> presets{};
        uint32_t current_kind{};
        std::string current_location;
        std::string current_load_location;
        std::string metadata_error;
        static bool surge_file(const std::filesystem::path& path) {
            std::ifstream file(path, std::ios::binary | std::ios::ate);
            auto size = file.tellg();
            if (size < 92) return false;
            unsigned char header[92]{};
            file.seekg(0);
            if (!file.read(reinterpret_cast<char*>(header), sizeof(header))) return false;
            auto equal = [&](int offset, const char* tag) { return std::memcmp(header + offset, tag, 4) == 0; };
            auto xml = uint32_t(header[64]) | uint32_t(header[65]) << 8 | uint32_t(header[66]) << 16 | uint32_t(header[67]) << 24;
            return equal(0, "CcnK") && equal(8, "FPCh") && equal(16, "cjs3") && equal(60, "sub3") && xml > 0 && xml <= 1024 * 1024 * 1024 && uint64_t(xml) <= uint64_t(size) - 92;
        }
        static bool valid_name(const char* name) {
#ifdef _WIN32
            return name && MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, name, -1, nullptr, 0) > 0;
#else
            return name != nullptr;
#endif
        }
        // Some Windows providers open CLAP UTF-8 locations with ANSI streams.
        // Retry only with an existing, losslessly encodable short alias.
        static std::string legacy_location(const std::filesystem::path& path) {
#ifdef _WIN32
            std::wstring short_path(32768, L'\\0');
            auto length = GetShortPathNameW(path.c_str(), short_path.data(), static_cast<DWORD>(short_path.size()));
            if (!length || length >= short_path.size()) return {};
            short_path.resize(length);
            BOOL substituted = FALSE;
            auto page = GetACP();
            auto flags = page == CP_UTF8 ? WC_ERR_INVALID_CHARS : WC_NO_BEST_FIT_CHARS;
            auto used = page == CP_UTF8 ? nullptr : &substituted;
            auto size = WideCharToMultiByte(page, flags, short_path.c_str(), -1, nullptr, 0, nullptr, used);
            if (!size || substituted) return {};
            std::string bytes(size, '\\0');
            if (!WideCharToMultiByte(page, flags, short_path.c_str(), -1, bytes.data(), size, nullptr, used) || substituted) return {};
            bytes.pop_back();
            return bytes;
#else
            return {};
#endif
        }
        static std::string utf8(const std::filesystem::path& path) {
            auto bytes = path.u8string();
            return {reinterpret_cast<const char*>(bytes.data()), bytes.size()};
        }''')
h = replace(h, 'std::cerr << "on_error: " << os_error << " " << error_message << std::endl;', '''auto* self = static_cast<PresetLoader*>(receiver->receiver_data);
            self->metadata_error = error_message ? error_message : "CLAP preset metadata error";''')
h = replace(h, '.name = name,\n                    .load_key = load_key ? load_key : "",', '''.name = valid_name(name) ? name : utf8(std::filesystem::u8path(current_location).stem()),
                    .load_key = load_key ? load_key : "",
                    .location_kind = current_kind,
                    .location = current_location,''')
h = replace(h, 'PresetLoader(clap_preset_discovery_factory* factory) {', '''PresetLoader(clap_preset_discovery_factory* factory) { }
        void discover(clap_preset_discovery_factory* factory) {''')
h = replace(h, 'auto provider = factory->create(factory, &indexer, desc->id);\n                providers.emplace_back(provider);\n                provider->init(provider);', '''locations.clear();
                filetypes.clear();
                auto provider = factory->create(factory, &indexer, desc->id);
                if (!provider) throw std::runtime_error("CLAP preset provider creation failed");
                providers.emplace_back(provider);
                if (!provider->init(provider)) throw std::runtime_error("CLAP preset provider init failed");''')
h = replace(h, 'std::filesystem::path path(location.location);', 'auto path = std::filesystem::u8path(location.location);')
h = replace(h, 'f.path().extension().string()', 'utf8(f.path().extension())')
h = replace(h, 'if (!provider->get_metadata(provider, location.kind, f.path().string().c_str(), &receiver)) {', '''current_kind = location.kind;
                                    current_location = utf8(f.path());
                                    current_load_location = current_location;
                                    metadata_error.clear();
                                    auto first = presets.size();
                                    auto ok = provider->get_metadata(provider, current_kind, current_load_location.c_str(), &receiver);
                                    if (!ok || !metadata_error.empty()) {
                                        presets.resize(first);
                                        auto alias = legacy_location(f.path());
                                        if (!alias.empty() && alias != current_load_location) {
                                            current_load_location = std::move(alias);
                                            metadata_error.clear();
                                            ok = provider->get_metadata(provider, current_kind, current_load_location.c_str(), &receiver);
                                        }
                                    }
                                    if ((!ok || !metadata_error.empty()) &&
                                        std::string_view(desc->id) == "org.surge-synth-team.surge-xt.preset-indexer" && surge_file(f.path())) {
                                        presets.resize(first);
                                        begin_preset(nullptr, nullptr);
                                        metadata_error.clear();
                                        ok = true;
                                    }
                                    if (!ok || !metadata_error.empty()) {
                                        presets.resize(first);''')
h = replace(h, 'f.path().string()', 'utf8(f.path())')
h = replace(h, 'provider->get_metadata(provider, location.kind, nullptr, &receiver);', '''current_kind = location.kind;
                        current_location.clear();
                        current_load_location.clear();
                        metadata_error.clear();
                        auto first = presets.size();
                        if (!provider->get_metadata(provider, location.kind, nullptr, &receiver) || !metadata_error.empty())
                            presets.resize(first);''')
h = replace(h, 'auto presets = loader->presets;', 'loader->discover(factory);\n            auto presets = loader->presets;')
h = replace(h, 'if (!ok || !metadata_error.empty()) {\n                                        presets.resize(first);\n                                        remidy', 'if (!ok || !metadata_error.empty()) {\n                                        presets.resize(first);\n                                        throw std::runtime_error("CLAP preset metadata: " + current_location + ": " + metadata_error);\n                                        remidy')
h = replace(h, '\n                            presets.resize(first);', '\n                            { presets.resize(first); throw std::runtime_error("CLAP preset metadata: " + metadata_error); }')
texts['CLAPHelper.hpp'] = '#include <stdexcept>\n#include <fstream>\n#include <cstring>\n#ifdef _WIN32\n#include <Windows.h>\n#endif\n' + h
p = texts['PluginInstanceCLAP.Presets.cpp']
p = replace(p, 'if (!owner->plugin || !owner->plugin->canUsePresetLoad())\n        return;', '''if (!owner->plugin || !owner->plugin->canUsePresetLoad())
        throw std::runtime_error("preset loading is not supported");
    if (index < 0 || index >= static_cast<int32_t>(presets.size()))
        throw std::runtime_error("invalid preset index");''')
p = replace(p, 'EventLoop::runTaskOnMainThread([&] {', 'EventLoop::runTaskOnMainThread([this, index] {')
p = replace(p, 'owner->plugin->presetLoadFromLocation(CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN, nullptr, presets[index].load_key)', '''owner->plugin->presetLoadFromLocation(presets[index].location_kind,
                presets[index].location_kind == CLAP_PRESET_DISCOVERY_LOCATION_FILE ? presets[index].location.c_str() : nullptr,
                presets[index].load_key.empty() ? nullptr : presets[index].load_key.c_str())''',2)
load_call = '''owner->plugin->presetLoadFromLocation(presets[index].location_kind,
                presets[index].location_kind == CLAP_PRESET_DISCOVERY_LOCATION_FILE ? presets[index].location.c_str() : nullptr,
                presets[index].load_key.empty() ? nullptr : presets[index].load_key.c_str())'''
p = replace(p, load_call, 'loadFromLocation(index)', 2)
p += '\n#include "' + (Path(__file__).parent / 'stopped-surge.hpp').resolve().as_posix() + '"\n'
header = texts['PluginFormatCLAP.hpp']
header = replace(header, '        std::atomic<bool> flush_requested_{false};',
                 '        std::atomic<uint64_t> preset_dirty_generation_{0};\n        std::atomic<bool> flush_requested_{false};')
header = replace(header, '            explicit PresetsSupport(PluginInstanceCLAP* owner);',
                 '            bool loadFromLocation(int32_t index);\n            explicit PresetsSupport(PluginInstanceCLAP* owner);')
texts['PluginFormatCLAP.hpp'] = header
host_header = texts['HostClasses.hpp']
host_header = replace(host_header, '        // clap_host_params', '''        bool implementsState() const noexcept override { return true; }
        void stateMarkDirty() noexcept override;

        // clap_host_params''')
texts['HostClasses.hpp'] = host_header
host = texts['HostClasses.cpp']
host = replace(host, '    void RemidyCLAPHost::paramsRescan(', '''    void RemidyCLAPHost::stateMarkDirty() noexcept {
        if (auto* instance = attached_instance.load())
            instance->preset_dirty_generation_.fetch_add(1);
    }

    void RemidyCLAPHost::paramsRescan(''')
texts['HostClasses.cpp'] = host
# Keep exceptions inside the queued asynchronous job, and always complete it.
p = replace(p, '        auto error = std::string{};', '        auto error = std::string{};\n        try {')
p = replace(p, '        if (completed)\n            completed(std::move(error), nullptr);', '''        } catch (const std::exception& failure) { error = failure.what(); }
          catch (...) { error = "Unknown CLAP preset load error"; }
        if (completed)
            completed(std::move(error), nullptr);''')
texts['PluginInstanceCLAP.Presets.cpp'] = p

# App-owned diagnostics use the host catalog; upstream matching anchors stay intact.
def localize_generated(text):
    messages = {'CLAP preset metadata error': 'patch_00', 'CLAP preset provider creation failed': 'patch_01', 'CLAP preset provider init failed': 'patch_02', 'CLAP preset metadata: ': 'patch_03', 'preset loading is not supported': 'patch_04', 'invalid preset index': 'patch_05', 'Unknown CLAP preset load error': 'patch_06', 'Unknown preset enumeration error': 'patch_07', 'Invalid instance ID or preset index': 'patch_08', 'Unknown preset discovery error': 'patch_09', 'Stopped Surge preset workaround requires verified CLAP version 1.3.4': 'patch_10', 'Surge is still processing; retry after stopping finishes': 'patch_11', 'Surge preset flush requires a fully stopped, configured 32-bit instance': 'patch_12', 'Surge preset flush could not start isolated processing': 'patch_13', 'Surge preset isolated processing failed': 'patch_14', 'Surge preset load did not report actual state change within 10 seconds': 'patch_15'}
    import json
    for message, key in messages.items():
        text = text.replace(json.dumps(message), 'repl_localized("' + key + '")')
    text = text.replace('repl_localized("patch_03") +', 'std::string(repl_localized("patch_03")) +')
    text = replace(text, 'std::string(repl_localized("patch_03")) + current_location + ": " + metadata_error', 'repl_format1("patch_03", "detail", current_location + ": " + metadata_error)') if 'current_location + ": " + metadata_error' in text else text
    text = replace(text, 'std::string(repl_localized("patch_03")) + metadata_error', 'repl_format1("patch_03", "detail", metadata_error)') if 'std::string(repl_localized("patch_03")) + metadata_error' in text else text
    if 'repl_localized("patch_03")' in text:
        raise RuntimeError('unconverted CLAP metadata diagnostic template')
    header = (Path(__file__).resolve().parents[3] / 'native/localization.hpp').as_posix()
    return '#include "' + header + '"\n' + text

# All translation units must see the same augmented preset type.
for name, text in texts.items():
    text = re.sub(r'#include "(\.\./[^\"]+)"', lambda m: '#include "' + (source/m[1]).resolve().as_posix() + '"', text)
    (output/name).write_text(localize_generated(text),encoding='utf-8')
# Compose with script-api's generated runtime, never overwrite its host callback.
s = runtime.read_text(encoding='utf-8')
s = replace(s, 'choc::json::parse(job->resultJson)', 'choc::json::parseValue(job->resultJson)')
a = s.index('        auto presets = instance->presetMetadataList();')
b = s.index('\n    });',a)
body = s[a:b]
s = s[:a] + '        try {\n' + body + '''
        } catch (const std::exception& error) { repl_preset_error(error.what()); }
          catch (...) { repl_preset_error("Unknown preset enumeration error"); }
        return choc::value::createEmptyArray();''' + s[b:]
a = s.index('        auto& sequencer =', s.index('impl_->context.registerFunction ("__remidy_instance_load_preset"'))
s = s[:a] + '''        try {
            auto* target = uapmd_app::AppModel::instance().sequencer().engine()->getPluginInstance(instanceId);
            if (!target || presetIndex >= static_cast<int32_t>(target->presetMetadataList().size()))
                throw std::runtime_error("Invalid instance ID or preset index");
        } catch (const std::exception& error) {
            completeMutationJob(job, choc::value::createObject("PluginPresetResult"), error.what());
            return mutationJobValue(job);
        } catch (...) {
            completeMutationJob(job, choc::value::createObject("PluginPresetResult"), "Unknown preset discovery error");
            return mutationJobValue(job);
        }
''' + s[a:]
s = 'extern "C" void repl_preset_error(const char*);\n' + s
runtime.write_text(localize_generated(s),encoding='utf-8')
