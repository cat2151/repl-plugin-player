#pragma once
#include "../patches/uapmd/parameter-metadata/metadata.hpp"

inline choc::value::Value parameter_safety(int32_t instance_id, int32_t parameter_id) {
    auto* instance = uapmd_app::AppModel::instance().sequencer().engine()->getPluginInstance(instance_id);
    if (!instance || parameter_id < 0) throw std::runtime_error(repl_localized("ui_metadata"));
    repl_parameters::Capture capture(parameter_id);
    auto metadata = instance->parameterMetadataList();
    if (static_cast<size_t>(parameter_id) >= metadata.size())
        throw std::runtime_error(repl_localized("ui_metadata"));
    const auto& p = metadata[parameter_id];
    // UAPMD's get/set addresses vector indexes. Reject sparse upstream indexes.
    if (p.index != static_cast<uint32_t>(parameter_id))
        throw std::runtime_error(repl_localized("ui_metadata"));
    auto result = choc::value::createObject("");
    result.setMember("instanceId", instance_id);
    result.setMember("parameterId", parameter_id);
    result.setMember("stableId", p.stableId);
    result.setMember("name", p.name);
    result.setMember("path", p.path);
    result.setMember("min", p.minPlainValue);
    result.setMember("max", p.maxPlainValue);
    result.setMember("readonly", true); // Unknown formats remain disabled.
    result.setMember("discrete", p.discrete);
    if (auto info = repl_parameters::observed; info && std::to_string(info->stable_id) == p.stableId) {
        result.setMember("readonly", (info->flags & (1u << 3)) != 0);
        result.setMember("discrete", (info->flags & 1u) != 0);
        result.setMember("flags", static_cast<int64_t>(info->flags));
        result.setMember("epoch", static_cast<int64_t>(info->epoch));
    }
    return result;
}

// Native-to-native only; no JS/Rust reentry and no exceptions through QuickJS.
extern "C" bool repl_parameter_readonly(int32_t instance_id, int32_t parameter_id) {
    try { return parameter_safety(instance_id, parameter_id)["readonly"].get<bool>(); }
    catch (...) { return true; }
}
