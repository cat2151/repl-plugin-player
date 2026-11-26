// Plays a held note through the real engine and counts what reaches the track input.
#include <uapmd-engine/uapmd-engine.hpp>
#include <iostream>
#include <stdexcept>
#include <string>

// The app's bridge owns these observers.
void repl_capture_scope(int, remidy::AudioProcessContext&) noexcept {}

namespace {
constexpr int32_t kFrames = 256;

struct Recorder final : uapmd::AudioProcessingEventHandler {
    int ons{}, offs{};
    bool held{}, overlapped{};
    bool supportsParallelTrackProcessing() const noexcept override { return true; }
    void beforeTrackProcess(const uapmd::TrackAudioProcessingEvent& event) noexcept override {
        if (event.track_index != 0) return;
        auto& input = event.context.eventIn();
        const auto* data = static_cast<const uint32_t*>(input.getMessages());
        const auto words = input.position() / sizeof(uint32_t);
        for (size_t i = 0; i < words; i += (data[i] >> 28) == 4 ? 2 : 1) {
            if ((data[i] >> 28) != 4) continue;
            const auto status = (data[i] >> 20) & 15;
            if (status == 9) { ++ons; overlapped |= held; held = true; }
            if (status == 8) { ++offs; held = false; }
        }
    }
};

// The clip outlives the loop, so only the loop end can release the note.
bool releasesEveryPass(const char* name, int64_t start, int64_t end) {
    auto engine = uapmd::SequencerEngine::create(48000, kFrames, 65536);
    engine->setEngineActive(true);
    if (engine->addEmptyTrack() < 0) throw std::runtime_error("track not added");
    uapmd::ClipData clip;
    clip.clipType = uapmd::ClipType::Midi;
    clip.position.samples = start;
    clip.durationSamples = 480000;
    clip.sourceNodeInstanceId = 7;
    clip.setTimeReference(uapmd::TimeReference::fromContainerStart(
        {}, static_cast<double>(start) / 48000), 48000);
    auto node = std::make_unique<uapmd::MidiClipSourceNode>(7,
        std::vector<uapmd_ump_t>{0x40903c00u, 0x80000000u, 0x40803c00u, 0u},
        std::vector<uint64_t>{0, 0, 9600, 9600}, 480, 120.0, 48000.0,
        std::vector<uapmd::MidiTempoChange>{}, std::vector<uapmd::MidiTimeSignatureChange>{});
    if (engine->timeline().tracks()[0]->addClip(clip, std::move(node)) < 0)
        throw std::runtime_error("clip not added");
    Recorder recorder;
    engine->addAudioProcessingEventHandler(recorder);
    engine->timeline().loopRegion({true, start, end});
    engine->startPlayback();
    remidy::AudioProcessContext process(engine->data().masterContext(), 65536);
    process.configureMainBus(2, 2, kFrames);
    process.frameCount(kFrames);
    for (int block = 0; block < 64; ++block)
        if (engine->processAudio(process) != 0) throw std::runtime_error("processAudio failed");
    engine->stopPlayback();
    engine->removeAudioProcessingEventHandler(recorder);
    const bool ok = recorder.ons >= 4 && recorder.offs >= recorder.ons - 1 && !recorder.overlapped;
    std::cout << (ok ? "PASS " : "FAIL ") << name << ": ons " << recorder.ons
              << " offs " << recorder.offs << '\n';
    return ok;
}
}

int main() {
    try {
        bool ok = releasesEveryPass("loop end on a block boundary", 0, kFrames * 8);
        ok &= releasesEveryPass("nonzero start, end on a block boundary", kFrames * 2, kFrames * 8);
        ok &= releasesEveryPass("loop end inside a block", 0, kFrames * 8 + 100);
        return ok ? 0 : 1;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
