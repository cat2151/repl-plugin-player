// Included by the generated CLAP presets source; no dependency checkout edits.
#include <chrono>
#include <thread>

bool remidy::PluginInstanceCLAP::PresetsSupport::loadFromLocation(int32_t index) {
    const auto& selected = presets.at(index);
    auto load = [&] {
        return owner->plugin->presetLoadFromLocation(selected.location_kind,
            selected.location_kind == CLAP_PRESET_DISCOVERY_LOCATION_FILE ? selected.location.c_str() : nullptr,
            selected.load_key.empty() ? nullptr : selected.load_key.c_str());
    };
    const auto* descriptor = owner->plugin->clapPlugin()->desc;
    if (std::string(descriptor->id) != "org.surge-synth-team.surge-xt" ||
        selected.location_kind != CLAP_PRESET_DISCOVERY_LOCATION_FILE || owner->is_processing.load())
        return load();
    auto failure = [](const char* reason) {
        return std::runtime_error(repl_format1("patch_context", "error", reason));
    };
    if (!descriptor->version || std::string(descriptor->version) != "1.3.4")
        throw failure(repl_localized("patch_10"));

    auto expected = ProcessingState::Idle;
    if (!owner->processing_state_.compare_exchange_strong(expected, ProcessingState::Restarting))
        throw failure(repl_localized("patch_11"));
    struct Guard {
        PluginInstanceCLAP* owner;
        bool started{};
        ~Guard() {
            if (started) {
                AudioThreadScope audioThread;
                owner->plugin->stopProcessing();
                owner->plugin->reset();
            }
            owner->processing_state_.store(ProcessingState::Idle);
        }
    } guard{owner};
    if (owner->is_processing.load() || owner->processing_active_.load() || !owner->activated_ || owner->use_double_precision_)
        throw failure(repl_localized("patch_12"));

    // Own every audio pointer. No realtime buffers, engine, graph, MIDI, or
    // device callback participates; output samples/events are discarded.
    const auto frames = std::min(uint32_t{128}, owner->buffer_size_);
    struct Ports {
        std::vector<clap_audio_buffer_t> ports;
        std::vector<std::vector<std::vector<float>>> samples;
        std::vector<std::vector<float*>> channels;
        Ports(const std::vector<clap_audio_buffer_t>& layout, uint32_t count)
            : ports(layout.size()), samples(layout.size()), channels(layout.size()) {
            for (size_t i = 0; i < layout.size(); ++i) {
                samples[i].resize(layout[i].channel_count, std::vector<float>(count));
                for (auto& channel : samples[i]) channels[i].push_back(channel.data());
                ports[i].channel_count = layout[i].channel_count;
                ports[i].data32 = channels[i].data();
            }
        }
    } inputs(owner->audio_in_port_buffers, frames), outputs(owner->audio_out_port_buffers, frames);
    clap_input_events_t input{};
    input.size = [](const clap_input_events_t*) -> uint32_t { return 0; };
    input.get = [](const clap_input_events_t*, uint32_t) -> const clap_event_header_t* { return nullptr; };
    clap_output_events_t output{};
    output.try_push = [](const clap_output_events_t*, const clap_event_header_t*) { return true; };
    clap_event_transport_t transport{};
    transport.header.size = sizeof(transport);
    transport.header.type = CLAP_EVENT_TRANSPORT;
    transport.flags = CLAP_TRANSPORT_HAS_TEMPO | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;
    transport.tempo = 120;
    transport.tsig_num = transport.tsig_denom = 4;
    clap_process_t process{};
    process.frames_count = frames;
    process.steady_time = -1;
    process.transport = &transport;
    process.audio_inputs = inputs.ports.data();
    process.audio_inputs_count = static_cast<uint32_t>(inputs.ports.size());
    process.audio_outputs = outputs.ports.data();
    process.audio_outputs_count = static_cast<uint32_t>(outputs.ports.size());
    process.in_events = &input;
    process.out_events = &output;
    {
        AudioThreadScope audioThread;
        if (!owner->plugin->startProcessing())
            throw failure(repl_localized("patch_13"));
        guard.started = true;
    }
    auto pump = [&] {
        {
            AudioThreadScope audioThread;
            if (owner->plugin->process(&process) == CLAP_PROCESS_ERROR)
                throw failure(repl_localized("patch_14"));
        }
        owner->plugin->onMainThread();
    };
    // In verified 1.3.4 checkNamesEvery++ > 10 gates patchChanged exchange.
    // Twelve calls plus wrapper callbacks drain old notifications before load.
    // This count is NOT the success condition for the requested preset.
    for (int i = 0; i < 12; ++i) pump();
    const auto generation = owner->preset_dirty_generation_.load();
    if (!load()) return false;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    do {
        pump();
        if (owner->preset_dirty_generation_.load() != generation) return true;
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    } while (std::chrono::steady_clock::now() < deadline);
    throw failure(repl_localized("patch_15"));
}
