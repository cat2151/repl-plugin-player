#pragma once
#include <uapmd-engine/uapmd-engine.hpp>
#include <uapmd-engine/detail/sequencer/PlaybackEngineExtension.hpp>
#include <choc/text/choc_JSON.h>
#include <array>
#include <bit>
#include <cmath>
#include <memory>
#include <stdexcept>

// Prepared MIDI only. The upstream timeline owns wrapping and block splitting.
namespace repl_loop {
struct Event {
    int64_t sample{};
    uint32_t words[2]{};
    uint32_t size{};
};
struct Part {
    std::string track;
    std::vector<Event> events;
};
struct Plan {
    uint64_t revision{};
    int64_t length{};
    std::vector<Part> parts;
};
class Playback final : public uapmd::PlaybackEngineExtension {
    friend struct Regression;
    using Publisher = uapmd::RtSnapshotPublisher<Plan, 4>;
    Publisher plans;
    std::array<Publisher::Guard, 4> guards;
    int active{-1}, pending{-1}, previous{-1};
    std::atomic<const Plan*> visiblePlan{nullptr};
    std::atomic<int64_t> visiblePosition{0};
    int64_t boundary{}, lastEnd{};
    int64_t origin{}, previousOrigin{}, previousLength{};
    bool startedPending{};
    uint64_t revision{};
    uint64_t stoppedRevision{};
    std::atomic<int64_t> period{0};
    double bpm{120};
    int sampleRate{};
    // Per-track notes generated here; never panic unrelated clips or parts.
    std::array<std::array<std::array<uint64_t, 2>, 16>, 32> held{};
    std::array<uint64_t, 32> renderedRevision{};
    std::atomic<int64_t> appliedBoundary{-1};
    std::atomic<uint64_t> appliedRevision{0};
    std::atomic<uint64_t> dropped{0};

    bool append(remidy::EventSequence& out, const uint32_t* words, uint32_t size, uint32_t offset) {
        auto pos = out.position() / sizeof(uint32_t);
        if (offset > 65535 || (pos + size + 1) * sizeof(uint32_t) > out.maxMessagesInBytes()) {
            dropped.fetch_add(1, std::memory_order_relaxed);
            return false;
        }
        auto* buffer = static_cast<uint32_t*>(out.getMessages());
        buffer[pos++] = (0x20u << 16) | offset;
        for (uint32_t i = 0; i < size; ++i) buffer[pos++] = words[i];
        out.position(pos * sizeof(uint32_t));
        return true;
    }
    void flush(remidy::EventSequence& out, size_t track, uint32_t offset) {
        for (uint32_t ch = 0; ch < 16; ++ch)
            for (uint32_t half = 0; half < 2; ++half) {
                auto bits = held[track][ch][half];
                while (bits) {
                    const auto note = static_cast<uint32_t>(std::countr_zero(bits)) + half * 64;
                    const uint32_t word = 0x20800000u | (ch << 16) | (note << 8);
                    if (append(out, &word, 1, offset))
                        held[track][ch][half] &= ~(uint64_t{1} << (note % 64));
                    bits &= bits - 1;
                }
            }
    }
public:
    std::string_view extensionId() const override { return "repl-loop"; }
    static int64_t sampleLength(double beats, double tempo, int sr) {
        const double samples = beats * 60.0 / tempo * sr;
        if (!std::isfinite(beats) || !std::isfinite(tempo) || beats <= 0 || tempo <= 0 ||
            !std::isfinite(samples) || samples < 1 || samples > 1e12)
            throw std::runtime_error("Invalid loop length or tempo");
        return static_cast<int64_t>(std::llround(samples));
    }
    void configure(double beats, double tempo, uapmd::SequencerEngine& engine) {
        const auto sr = engine.currentSampleRate();
        const auto length = sampleLength(beats, tempo, sr);
        if (period.load() == length && bpm == tempo && sampleRate == sr) return;
        if (engine.isPlaybackActive()) throw std::runtime_error("Stop before changing loop length or tempo");
        // Transport mutations already exclude the audio callback in UAPMD.
        engine.resetTrackProcessingState(0, false, [&] {
            playbackStopped();
            plans.publish(std::make_unique<const Plan>());
            bpm = tempo;
            sampleRate = sr;
            period.store(length);
        });
    }
    // Also clears the transport loop that begin() wrote, so upstream wrapping stops too.
    void clear(uapmd::SequencerEngine& engine) {
        auto& state = engine.timeline().state();
        if (period.load() == 0 && !state.loopEnabled) return;
        if (engine.isPlaybackActive()) throw std::runtime_error("Stop before clearing the loop");
        engine.resetTrackProcessingState(0, false, [&] {
            playbackStopped();
            plans.publish(std::make_unique<const Plan>());
            period.store(0);
            state.loopEnabled = false;
            state.loopStart.samples = 0;
            state.loopEnd.samples = 0;
        });
    }
    uint64_t publish(const choc::value::ValueView& input, uapmd::SequencerEngine& engine,
                     double beats = 0, double tempo = 0) {
        const auto sr = engine.currentSampleRate();
        const auto candidateTempo = tempo == 0 ? bpm : tempo;
        const auto length = beats == 0 ? period.load() : sampleLength(beats, candidateTempo, sr);
        if (length <= 0) throw std::runtime_error("Configure the common loop first");
        if (engine.isPlaybackActive() && (candidateTempo != bpm || sr != sampleRate))
            throw std::runtime_error("Stop before changing tempo or sample rate");
        if (!input.isArray() || input.size() > 32) throw std::runtime_error("Expected at most 32 prepared loop parts");
        auto next = std::make_unique<Plan>();
        next->length = length;
        auto tracks = engine.timeline().tracks();
        for (const auto& item : input) {
            const auto index = item["trackIndex"].get<int32_t>();
            if (index < 0 || index >= 32 || static_cast<size_t>(index) >= tracks.size())
                throw std::runtime_error("Invalid loop track");
            Part part;
            part.track = tracks[index]->referenceId();
            if (std::any_of(next->parts.begin(), next->parts.end(), [&](const Part& p) { return p.track == part.track; }))
                throw std::runtime_error("Combine parts on the same track in script before publishing");
            auto info = uapmd::MidiClipReader::readAnyFormat(std::filesystem::u8path(item["filepath"].get<std::string>()));
            if (!info.success || !info.tick_resolution || info.ump_data.size() != info.ump_tick_timestamps.size())
                throw std::runtime_error("Invalid loop MIDI: " + info.error);
            for (const auto& tempo : info.tempo_changes)
                if (std::abs(tempo.bpm - candidateTempo) > 0.001) throw std::runtime_error("Loop MIDI must use the common fixed tempo");
            // Reuse upstream's SMF conversion and tick-to-sample conversion.
            uapmd::MidiClipSourceNode source(-1, std::move(info.ump_data), std::move(info.ump_tick_timestamps),
                info.tick_resolution, candidateTempo, sr, std::move(info.tempo_changes), std::move(info.time_signature_changes));
            const auto& words = source.umpEvents();
            const auto& times = source.eventTimestampsSamples();
            for (size_t i = 0; i < words.size();) {
                const auto type = words[i] >> 28;
                const auto size = static_cast<size_t>(umppi::umpSizeInInts(type));
                if (size == 0 || size > words.size() - i) throw std::runtime_error("Malformed loop UMP");
                if (type == 2 || type == 4) {
                    const auto status = (words[i] >> 20) & 15;
                    if (((words[i] >> 24) & 15) || (status != 8 && status != 9))
                        throw std::runtime_error("Loop clips accept group-zero notes only");
                    if (times[i] < static_cast<uint64_t>(length)) {
                        Event event{static_cast<int64_t>(times[i]), {words[i], size == 2 ? words[i + 1] : 0}, static_cast<uint32_t>(size)};
                        part.events.push_back(event);
                    }
                }
                i += size;
            }
            if (part.events.size() > 16384) throw std::runtime_error("Loop part event limit exceeded");
            next->parts.push_back(std::move(part));
        }
        // No timing or published state changes until every part has validated.
        if (!engine.isPlaybackActive()) {
            bpm = candidateTempo;
            sampleRate = sr;
            period.store(length);
        }
        next->revision = ++revision;
        plans.publish(std::move(next));
        return revision;
    }
    void playbackStarted() override {
        visiblePlan.store(nullptr);
        visiblePosition.store(0);
        for (auto& guard : guards) guard = {};
        active = pending = previous = -1;
        lastEnd = 0;
        origin = previousOrigin = previousLength = 0;
        held = {};
        renderedRevision = {};
    }
    void playbackStopped() override {
        visiblePlan.store(nullptr);
        // Cancel unpublished/in-flight reservations. No delayed operation can restart transport.
        if (active >= 0) {
            stoppedRevision = guards[active]->revision;
            period.store(guards[active]->length > 0 ? guards[active]->length : period.load());
            plans.publish(std::make_unique<const Plan>(*guards[active]));
        }
        else if (plans.currentOnPublisherThread()->revision != stoppedRevision)
            plans.publish(std::make_unique<const Plan>());
        playbackStarted();
    }
    void begin(uapmd::SequencerEngine& engine, uapmd::TimelineState& state, int32_t frames) {
        const auto length = period.load();
        if (length <= 0) return;
        if (!engine.isPlaybackActive()) return;
        const auto raw = engine.renderPlaybackPosition();
        state.tempo = bpm;
        int64_t maxStart = raw;
        for (size_t i = 0; i < engine.tracks().size(); ++i) {
            const auto lead = engine.trackRenderLeadInSamples(static_cast<int32_t>(i)) + engine.masterTrackRenderLeadInSamples();
            maxStart = std::max(maxStart, raw + lead);
        }
        advance(raw, maxStart, frames);
        state.loopEnabled = true;
        state.loopStart.samples = 0;
        state.loopEnd.samples = timing(engine.playbackPosition()).first;
        captureView(engine.playbackPosition());
    }
private:
    void captureView(int64_t audible) {
        visiblePosition.store(position(audible));
        const auto index = pending >= 0 && audible >= boundary ? pending
            : (audible < origin && previous >= 0 ? previous : active);
        visiblePlan.store(index >= 0 ? guards[index].get() : nullptr);
    }
    void advance(int64_t raw, int64_t maxStart, int32_t frames) {
        // Clear before releasing any guard. Only the control thread reclaims
        // plans, so it can serialize a non-null pointer without audio locks.
        visiblePlan.store(nullptr);
        if (pending >= 0 && raw >= boundary) {
            previousOrigin = origin;
            previousLength = guards[active]->length > 0 ? guards[active]->length : period.load();
            origin = boundary;
            if (previous >= 0) guards[previous] = {};
            previous = active;
            active = pending;
            pending = -1;
        }
        int free = 0;
        while (free == active || free == pending || free == previous) ++free;
        guards[free] = plans.protect(free);
        if (active < 0) { active = free; free = -1; }
        else if (guards[free]->revision != guards[active]->revision &&
                 (pending < 0 || (!startedPending && guards[free]->revision != guards[pending]->revision))) {
            if (pending >= 0) guards[pending] = {};
            pending = free;
            free = -1;
            const auto firstAvailable = std::max(maxStart, lastEnd);
            const auto length = guards[active]->length > 0 ? guards[active]->length : period.load();
            boundary = origin + ((std::max<int64_t>(0, firstAvailable - origin) + length - 1) / length) * length;
            startedPending = false;
            appliedBoundary.store(boundary);
        }
        if (free >= 0) guards[free] = {};
        if (pending >= 0 && maxStart + frames > boundary) startedPending = true;
        lastEnd = maxStart + frames;
    }
public:
    // Control/publishing thread only: no publish/reclamation during the read.
    // The audio thread retains the preceding plan for the audible playhead,
    // which can lag the render head by a device buffer or plug-in latency.
    choc::value::Value view(bool playing) const {
        const auto* plan = playing && period.load() > 0
            ? visiblePlan.load() : plans.currentOnPublisherThread();
        auto result = choc::value::createObject("", "ready", plan != nullptr,
            "positionSamples", visiblePosition.load(), "bpm", bpm);
        if (!plan) return result;
        result.setMember("revision", static_cast<int64_t>(plan->revision));
        result.setMember("lengthSamples", plan->length > 0 ? plan->length : period.load());
        auto parts = choc::value::createEmptyArray();
        for (const auto& part : plan->parts) {
            auto events = choc::value::createEmptyArray();
            for (const auto& event : part.events) {
                auto words = choc::value::createEmptyArray();
                for (uint32_t i = 0; i < event.size; ++i) words.addArrayElement(static_cast<int64_t>(event.words[i]));
                events.addArrayElement(choc::value::createObject("", "sample", event.sample, "words", words));
            }
            parts.addArrayElement(choc::value::createObject("", "trackId", part.track, "events", events));
        }
        result.setMember("parts", parts);
        if (playing && period.load() > 0 && visiblePlan.load() != plan)
            result.setMember("ready", false);
        return result;
    }
    bool enabled() const { return active >= 0 && period.load() > 0; }
    // Scalars suffice for audible transport behind the active render plan.
    std::pair<int64_t, int64_t> timing(int64_t raw) const {
        if (pending >= 0 && raw >= boundary)
            return {guards[pending]->length > 0 ? guards[pending]->length : period.load(), boundary};
        if (raw < origin && previousLength > 0) return {previousLength, previousOrigin};
        return {active >= 0 && guards[active]->length > 0 ? guards[active]->length : period.load(), origin};
    }
    int64_t position(int64_t raw) const {
        if (raw < 0) return raw;
        const auto [length, start] = timing(raw);
        return length > 0 ? (raw - start) % length : raw;
    }
    int64_t span(int64_t raw) const {
        const auto [length, start] = timing(raw);
        auto count = raw < 0 ? -raw : length - (raw - start) % length;
        if (pending >= 0 && raw < boundary) count = std::min(count, boundary - raw);
        return count;
    }
    void segment(uapmd::TimelineTrack& track, size_t index, remidy::EventSequence& out,
                 int64_t raw, int64_t position, int32_t frames, uint32_t offset, bool playing) {
        if (!playing || active < 0 || index >= held.size()) return;
        // Upstream clamps negative preroll positions to zero for timeline clips.
        // Do not repeat the first note while a lower-latency track is still prerolling.
        if (raw < 0) {
            const auto skip = static_cast<int32_t>(std::min<int64_t>(-raw, frames));
            raw += skip;
            offset += skip;
            frames -= skip;
        }
        if (frames <= 0) return;
        while (frames > 0) {
            position = this->position(raw);
            const auto count = static_cast<int32_t>(std::min<int64_t>(frames, span(raw)));
            emitSegment(track, index, out, raw, position, count, offset);
            raw += count;
            offset += count;
            frames -= count;
        }
    }
private:
    void emitSegment(uapmd::TimelineTrack& track, size_t index, remidy::EventSequence& out,
                     int64_t raw, int64_t position, int32_t frames, uint32_t offset) {
        const auto* plan = guards[active].get();
        if (pending >= 0 && raw >= boundary) {
            plan = guards[pending].get();
            appliedRevision.store(plan->revision);
        }
        if (position == 0 || renderedRevision[index] != plan->revision) flush(out, index, offset);
        renderedRevision[index] = plan->revision;
        auto part = std::find_if(plan->parts.begin(), plan->parts.end(), [&](const Part& p) { return p.track == track.referenceId(); });
        if (part == plan->parts.end()) return;
        auto first = std::lower_bound(part->events.begin(), part->events.end(), position,
            [](const Event& e, int64_t sample) { return e.sample < sample; });
        for (; first != part->events.end() && first->sample < position + frames; ++first) {
            if (!append(out, first->words, first->size, offset + static_cast<uint32_t>(first->sample - position))) continue;
            const auto word = first->words[0];
            const auto note = (word >> 8) & 127;
            const auto velocity = first->size == 1 ? word & 127 : first->words[1] >> 16;
            auto& bits = held[index][(word >> 16) & 15][note / 64];
            const auto mask = uint64_t{1} << (note % 64);
            if (((word >> 20) & 15) == 9 && velocity) bits |= mask;
            else bits &= ~mask;
        }
    }
public:
    choc::value::Value status() const {
        return choc::value::createObject("", "revision", static_cast<int64_t>(appliedRevision.load()),
            "boundary", appliedBoundary.load(), "dropped", static_cast<int64_t>(dropped.load()));
    }
};
inline Playback* instance{}; // Session-owned; changed only with processing stopped.
}
