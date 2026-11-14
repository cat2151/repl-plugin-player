#pragma once
#include <set>

// Note cut and paste through uapmd's piano roll session for a clip named by its
// document id. Each call commits, so the project always holds the result.
namespace repl_piano_roll {

inline std::shared_ptr<uapmd_app::PianoRollSession> open(const std::string& clipId) {
    auto& model = uapmd_app::AppModel::instance();
    const auto clip = model.sequencer().engine()->document().view().getClip(clipId);
    if (!clip) throw std::runtime_error("No clip " + clipId);
    auto session = model.openPianoRollSession(clip->trackIndex, clip->clipNumericId);
    if (!session || !session->rawMidiData) throw std::runtime_error("No MIDI content in clip " + clipId);
    // The ids scripts read from uapmd.timeline.getClipNotes.
    session->content->ensureIdentities();
    return session;
}

inline void commit(uapmd_app::PianoRollSession& session) {
    if (!session.commit(uapmd_app::AppModel::instance()))
        throw std::runtime_error("Piano roll edit was not saved: " + session.edit_error);
}

// Selects the notes `noteIds` names and cuts them; returns how many were cut.
inline int64_t cut(const std::string& clipId, const choc::value::ValueView& noteIds) {
    auto session = open(clipId);
    std::set<std::string> wanted;
    for (const auto& id : noteIds) wanted.insert(id.get<std::string>());
    session->selectNote(-1);
    int64_t selected = 0;
    for (size_t i = 0; i < session->editNotes.size(); ++i)
        if (!session->editNotes[i].deleted && wanted.erase(session->editNotes[i].reference_id)) {
            session->selectNote(static_cast<int>(i), true);
            ++selected;
        }
    if (!wanted.empty()) throw std::runtime_error("No note " + *wanted.begin() + " in clip " + clipId);
    session->performAction(uapmd_app::PianoRollSession::Action::Cut, 0);
    commit(*session);
    return selected;
}

// Pastes the clipboard so its first note starts at `tick`; returns the new notes' ids.
inline choc::value::Value paste(const std::string& clipId, int64_t tick) {
    auto session = open(clipId);
    if (session->clipboard.empty()) throw std::runtime_error("Piano roll clipboard is empty");
    if (tick < 0) throw std::runtime_error("Paste tick must not be negative");
    // The same tick-to-seconds conversion uapmd uses when it loads a clip.
    const auto& raw = *session->rawMidiData;
    const double seconds = static_cast<double>(tick) * 60.0 /
        (std::max(1.0, raw.clipTempo) * std::max<uint32_t>(1, raw.tickResolution));
    std::set<std::string> before;
    for (const auto& note : session->editNotes)
        if (!note.deleted) before.insert(note.reference_id);
    session->performAction(uapmd_app::PianoRollSession::Action::Paste, seconds);
    commit(*session);
    auto added = choc::value::createEmptyArray();
    for (const auto& note : session->editNotes)
        if (!note.deleted && !before.contains(note.reference_id)) added.addArrayElement(note.reference_id);
    return added;
}

}

// `pianoRoll.*` script calls; nullptr for any other api.
static const char* piano_roll_api(const char* request, bool open) {
    static thread_local std::string result;
    try {
        const auto args = choc::json::parse(request);
        const auto api = args["api"].get<std::string>();
        if (!api.starts_with("pianoRoll.")) return nullptr;
        if (!open) throw std::runtime_error("No session");
        const auto clipId = args["clipId"].get<std::string>();
        auto value = choc::value::Value();
        if (api == "pianoRoll.cut") value = choc::value::createInt64(repl_piano_roll::cut(clipId, args["noteIds"]));
        else if (api == "pianoRoll.paste") value = repl_piano_roll::paste(clipId, args["tick"].get<int64_t>());
        else throw std::runtime_error("Unknown piano roll API");
        result = choc::json::toString(choc::value::createObject("", "ok", true, "value", value));
    } catch (const std::exception& error) {
        result = choc::json::toString(choc::value::createObject("", "ok", false, "error", error.what()));
    }
    return result.c_str();
}
