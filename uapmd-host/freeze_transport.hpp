#pragma once

// Model-thread polling: finish a finite score when freezing is waiting for Stop.
// Pause preserves the end position and lets the normal tail/quiet path finish.
inline void repl_finish_pending_freeze() {
    auto& engine = *uapmd_app::AppModel::instance().sequencer().engine();
    if (!engine.isPlaybackActive() || engine.offlineRendering()) return;
    if (engine.timeline().loopRegion().enabled) return;
    auto& freeze = engine.frozenTrackManager();
    bool pending = false;
    for (size_t i = 0; i < engine.tracks().size(); ++i) {
        if (engine.trackHasLiveInput(static_cast<int32_t>(i))) return;
        pending |= freeze.freezePolicyForTrack(static_cast<int32_t>(i)) ==
                       uapmd::FrozenTrackManager::FreezePolicy::On &&
                   freeze.runtimeStateForTrack(static_cast<int32_t>(i)) ==
                       uapmd::FrozenTrackManager::RuntimeState::Live;
    }
    if (!pending) return;
    const auto bounds = engine.timeline().calculateContentBounds();
    if (bounds.hasContent && engine.playbackPosition() >= bounds.lastSample)
        engine.pausePlayback();
}
