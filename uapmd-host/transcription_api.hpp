#pragma once

#include <future>
#include <stop_token>

#include "PitchTranscription.hpp"

class PitchTranscription {
    struct Clip {
        uapmd::ProjectClipSnapshot source;
        std::vector<uapmd_pitch::TranscribedNote> notes;
    };

    std::stop_source stop;
    std::future<std::vector<Clip>> worker;
    std::vector<Clip> ready;

    static std::vector<uapmd_pitch::TranscribedNote> analyze(
            const uapmd::ProjectClipSnapshot& clip, std::stop_token token) {
        auto reader = uapmd::createAudioFileReaderFromPath(clip.filepath);
        if (!reader) throw std::runtime_error("Could not open audio: " + clip.filepath);
        const auto info = reader->getProperties();
        if (!info.sampleRate || !info.numChannels) return {};
        const auto toFrames = [&](int64_t samples) {
            const double frames = std::round(samples * info.sampleRate / clip.sampleRate);
            return static_cast<uint64_t>(std::clamp(frames, 0.0, double(info.numFrames)));
        };
        const auto start = toFrames(clip.sourceOffsetSamples);
        const auto count = std::min(toFrames(clip.durationSamples), info.numFrames - start);
        constexpr uint64_t block = 4096;
        std::vector<float> mono;
        mono.reserve(count);
        std::vector<std::vector<float>> channels(info.numChannels, std::vector<float>(block));
        std::vector<float*> dest;
        for (auto& channel : channels) dest.push_back(channel.data());
        for (uint64_t offset = 0; offset < count && !token.stop_requested(); offset += block) {
            const auto frames = std::min(block, count - offset);
            reader->readFrames(start + offset, frames, dest.data(), info.numChannels);
            for (uint64_t frame = 0; frame < frames; ++frame) {
                float sum = 0;
                for (const auto& channel : channels) sum += channel[frame];
                mono.push_back(sum / info.numChannels);
            }
        }
        if (token.stop_requested()) return {};
        return uapmd_pitch::transcribeMonoToNotes(mono, info.sampleRate, {},
            [token](double) { return !token.stop_requested(); });
    }

public:
    ~PitchTranscription() { stop.request_stop(); }

    choc::value::Value start(const uapmd::ProjectDocumentView& view) {
        if (worker.valid() || !ready.empty()) throw std::runtime_error("Transcription is in progress");
        std::vector<uapmd::ProjectClipSnapshot> clips;
        for (const auto& track : view.trackIds()) {
            for (const auto& id : view.clipIds(track)) {
                auto clip = view.getClip(id);
                if (clip && clip->clipType == uapmd::ClipType::Audio && !clip->filepath.empty()
                    && clip->durationSamples > 0 && clip->sampleRate > 0)
                    clips.push_back(std::move(*clip));
            }
        }
        if (clips.empty()) return choc::value::createBool(false);
        // Only snapshots and readers owned by this worker cross threads.
        worker = std::async(std::launch::async, [clips = std::move(clips), token = stop.get_token()] {
            std::vector<Clip> result;
            for (const auto& clip : clips) {
                if (token.stop_requested()) break;
                auto notes = analyze(clip, token);
                if (!notes.empty()) result.push_back({clip, std::move(notes)});
            }
            return result;
        });
        return choc::value::createBool(true);
    }

    choc::value::Value poll() {
        if (worker.valid()) {
            if (worker.wait_for(std::chrono::seconds(0)) != std::future_status::ready) return {};
            ready = worker.get();
        }
        auto tracks = choc::value::createEmptyArray();
        uapmd::ProjectObjectId last;
        for (const auto& clip : ready) {
            if (clip.source.trackId == last) continue;
            last = clip.source.trackId;
            tracks.addArrayElement(last);
        }
        return tracks;
    }

    void clear() { ready.clear(); }

    void write(uapmd::SequencerEngine& engine, const std::string& sourceId, const std::string& targetId) {
        auto& timeline = engine.timeline();
        const auto& view = engine.document().view();
        const auto target = view.getTrack(targetId);
        if (!target || target->trackIndex < 0 || sourceId == targetId)
            throw std::runtime_error("Invalid transcription destination");
        const auto& state = timeline.state();
        const auto bpm = state.tempo > 0.0 ? state.tempo : 120.0;
        const auto resolution = state.projectTickResolution > 0 ? state.projectTickResolution : 480;
        for (const auto& [source, notes] : ready) {
            if (source.trackId != sourceId) continue;
            const auto clip = view.getClip(source.clipId);
            if (!clip || clip->clipType != uapmd::ClipType::Audio || clip->trackId != sourceId
                || clip->filepath != source.filepath || clip->sampleRate != source.sampleRate
                || clip->sourceOffsetSamples != source.sourceOffsetSamples
                || clip->durationSamples != source.durationSamples)
                throw std::runtime_error("Source audio clip changed during transcription");
            auto [events, ticks] = uapmd_pitch::makeNoteClip(notes, resolution, bpm);
            auto added = timeline.addMidiClipToTrack(target->trackIndex, clip->position,
                std::move(events), std::move(ticks), resolution, bpm, {}, {}, clip->name);
            if (!added.success) throw std::runtime_error(added.error);
            engine.commands().resizeClip(target->trackIndex, added.clipId, clip->durationSamples,
                uapmd::ProjectMutationOrigin::Internal);
        }
    }
};
