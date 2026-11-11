// Behavioral fixtures for discovery ownership, UTF-8 and failure cleanup.
#include "CLAPHelper.hpp"
#include <fstream>
#include <stdexcept>

namespace {
std::string folder, expected;
int mode{}, destroyed{}, calls{};
const clap_preset_discovery_indexer_t* host{};
clap_preset_discovery_provider_descriptor_t descriptor{CLAP_VERSION, "fixture", "fixture", "fixture"};
void check(bool condition, const char* message) { if (!condition) throw std::runtime_error(message); }
std::string utf8(const std::filesystem::path& path) {
    auto s = path.u8string(); return {reinterpret_cast<const char*>(s.data()), s.size()};
}
bool init(const clap_preset_discovery_provider_t*) {
    if (mode == 3) return false;
    clap_preset_discovery_filetype_t type{"fixture", nullptr, "fxp"};
    host->declare_filetype(host, &type);
    auto location = folder + (mode == 1 ? "/missing" : "");
    clap_preset_discovery_location_t where{0, "fixture", CLAP_PRESET_DISCOVERY_LOCATION_FILE, location.c_str()};
    host->declare_location(host, &where);
    return true;
}
void destroy(const clap_preset_discovery_provider_t*) { ++destroyed; }
bool metadata(const clap_preset_discovery_provider_t*, uint32_t kind, const char* path,
              const clap_preset_discovery_metadata_receiver_t* receiver) {
    ++calls;
    if (mode == 2) { receiver->on_error(receiver, 0, "fixture metadata failure"); return false; }
    check(kind == CLAP_PRESET_DISCOVERY_LOCATION_FILE && path == expected, "UTF-8 location corrupted");
    char key[] = "owned-key";
    const char* name = mode == 4 ? nullptr : mode == 5 ? "\xff" : reinterpret_cast<const char*>(u8"日本語の音色");
    receiver->begin_preset(receiver, name, key);
    key[0] = '!'; // Receiver must own the original key before returning.
    return true;
}
clap_preset_discovery_provider_t provider{&descriptor, nullptr, init, destroy, metadata, nullptr};
uint32_t count(const clap_preset_discovery_factory_t*) { return 1; }
const clap_preset_discovery_provider_descriptor_t* desc(const clap_preset_discovery_factory_t*, uint32_t) { return &descriptor; }
const clap_preset_discovery_provider_t* create(const clap_preset_discovery_factory_t*,
        const clap_preset_discovery_indexer_t* indexer, const char*) { host = indexer; return &provider; }
clap_preset_discovery_factory_t factory{count, desc, create};
}
int main(int argc, char** argv) {
    try {
        check(argc == 2, "provide an external temporary folder");
        auto root = std::filesystem::u8path(argv[1]);
        std::filesystem::create_directories(root);
        auto path = root / std::filesystem::path(u8"日本語 µ.fxp");
        folder = utf8(root); expected = utf8(path);
        std::ofstream(path).put('x');
        for (int selected : {0, 4, 5, 1, 2, 3, 0}) {
            mode = selected; destroyed = calls = 0;
            bool failed = false;
            try {
                auto [loader, presets] = remidy_clap::PresetLoader::createWithPresets(&factory);
                check(mode == 0 || mode == 4 || mode == 5, "failure was hidden");
                check(presets.size() == 1, "missing/duplicate preset");
                check(presets[0].name == (mode >= 4 ? utf8(path.stem()) : reinterpret_cast<const char*>(u8"日本語の音色")), "name corrupted");
                check(presets[0].load_key == "owned-key", "borrowed key retained");
                check(presets[0].location == expected, "location not retained");
                check(presets[0].location_kind == CLAP_PRESET_DISCOVERY_LOCATION_FILE, "location kind lost");
            } catch (const std::exception&) {
                failed = true;
                if (mode == 0 || mode == 4 || mode == 5) throw;
            }
            check(failed == (mode >= 1 && mode <= 3), "unexpected discovery result");
            check(destroyed == 1, "provider leaked on success/failure");
        }
        std::cout << "PASS discovery: UTF-8 names/path, optional name, owned keys/location, failure cleanup and retry\n";
        return 0;
    } catch (const std::exception& error) { std::cerr << error.what() << '\n'; return 1; }
}

extern "C" const char* repl_localized(const char* key) { return key; }
