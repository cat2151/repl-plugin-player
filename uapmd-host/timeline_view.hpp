#pragma once
#include <unordered_map>
#include <unordered_set>

// Private display metadata, not a second project or playback model.
class TimelineView {
    std::unordered_set<std::string> implicit;
    std::unordered_set<std::string> used;
public:
    void captureInitial(uapmd::SequencerEngine& engine) {
        for (const auto* track : engine.timeline().tracks()) implicit.insert(track->referenceId());
    }
    choc::value::Value read(uapmd::SequencerEngine& engine) {
        auto tracks = choc::value::createEmptyArray();
        for (const auto* track : engine.timeline().tracks()) {
            const auto& id = track->referenceId();
            auto clips = choc::value::createEmptyArray();
            // Scripts name clips by document id; the projection pairs them by it.
            std::unordered_map<int32_t, std::string> clipIds;
            const auto& document = engine.document().view();
            for (const auto& clipId : document.clipIds(id))
                if (const auto snapshot = document.getClip(clipId)) clipIds[snapshot->clipNumericId] = snapshot->clipId;
            const auto allClips = track->clipManager().getAllClips();
            if (!allClips.empty() || !engine.tracks()[tracks.size()]->orderedInstanceIds().empty()) used.insert(id);
            for (const auto& clip : allClips) {
                if (clip.clipType != uapmd::ClipType::Midi) continue;
                auto node = const_cast<uapmd::TimelineTrack*>(track)->getSourceNode(clip.sourceNodeInstanceId);
                auto* midi = dynamic_cast<uapmd::MidiClipSourceNode*>(node.get());
                if (!midi) continue;
                const auto& words = midi->umpEvents();
                const auto& samples = midi->eventTimestampsSamples();
                auto events = choc::value::createEmptyArray();
                for (size_t i = 0; i < words.size() && i < samples.size();) {
                    const auto count = static_cast<size_t>(umppi::umpSizeInInts(words[i] >> 28));
                    if (!count || count > words.size() - i) break;
                    auto message = choc::value::createEmptyArray();
                    for (size_t j = 0; j < count; ++j) message.addArrayElement(static_cast<int64_t>(words[i + j]));
                    events.addArrayElement(choc::value::createObject("", "sample", static_cast<int64_t>(samples[i]), "words", message));
                    i += count;
                }
                clips.addArrayElement(choc::value::createObject("", "clipId", clipIds[clip.clipId], "events", events));
            }
            tracks.addArrayElement(choc::value::createObject("", "trackId", id,
                "implicit", implicit.contains(id) && !used.contains(id), "clips", clips));
        }
        const auto metadata = engine.timeline().buildMasterTrackSnapshot();
        auto tempos = choc::value::createEmptyArray();
        for (const auto& point : metadata.tempoPoints)
            tempos.addArrayElement(choc::value::createObject("", "seconds", point.timeSeconds, "bpm", point.bpm));
        auto signatures = choc::value::createEmptyArray();
        for (const auto& point : metadata.timeSignaturePoints)
            signatures.addArrayElement(choc::value::createObject("", "seconds", point.timeSeconds,
                "numerator", static_cast<int32_t>(point.signature.numerator),
                "denominator", static_cast<int32_t>(point.signature.denominator)));
        return choc::value::createObject("", "tracks", tracks, "tempos", tempos, "signatures", signatures);
    }
};
