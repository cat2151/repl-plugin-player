"""Add bounded frozen-playback observations to generated source and headers."""
from pathlib import Path
import re
import subprocess
import sys

NAME = 'freeze-monitor'
REVISION = '9f708d3300b4a6f8109eee0fadccf286db8b7a45'


def once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'expected one anchor, found {source.count(old)}: {old}')
    return source.replace(old, new)


def transform(source):
    source = once(source, '''    AudioProcessContext&) {
    const auto* snapshot =
        active_playback_snapshot_.load(std::memory_order_acquire);
    if (!snapshot || trackIndex < 0 ||''', '''    AudioProcessContext&) {
    const auto* snapshot =
        active_playback_snapshot_.load(std::memory_order_acquire);
    // Retained states can outlive a removed track. Release their keyboard slots
    // on the audio thread, where the optional observers are exclusively owned.
    if (snapshot && trackIndex == 0 && !snapshot->repl_retired_cleared) {
        for (auto* retired : snapshot->repl_retired) retired->repl_note_state.reset();
        snapshot->repl_retired_cleared = true;
    }
    if (!snapshot || trackIndex < 0 ||''')
    source = once(source, '    const auto* published = snapshot.get();', '''    for (const auto& [_, state] : playback_states_by_track_reference_)
        if (state && std::find(snapshot->tracks.begin(), snapshot->tracks.end(), state.get()) == snapshot->tracks.end())
            snapshot->repl_retired.push_back(state.get());
    const auto* published = snapshot.get();''')
    source = once(source, '    const auto* state = snapshot->tracks[static_cast<size_t>(trackIndex)];',
                  '    auto* state = snapshot->tracks[static_cast<size_t>(trackIndex)];')
    source = once(source, '    return state &&\n', '    const bool frozen = state &&\n')
    source = once(source, '        state->cached_audio.load(std::memory_order_acquire);\n}',
                  '        state->cached_audio.load(std::memory_order_acquire);\n'
                  '    if (state && !frozen) state->repl_note_state.reset();\n'
                  '    return frozen;\n}')
    source = once(source, '    SequencerTrack&,\n    AudioProcessContext& context) {',
                  '    SequencerTrack& track,\n    AudioProcessContext& context) {')
    return once(source, '    state->cached_audio_in_use.store(nullptr, std::memory_order_release);', '''    // No plugin DSP is run: observe the scheduled input and cached track output.
    if (engine.isPlaybackActive()) {
        if (!state->repl_note_state) state->repl_note_state.emplace();
        const auto& ids = track.orderedInstanceIds();
        const auto target = repl_notes::target.load(std::memory_order_relaxed);
        const bool selected = std::find(ids.begin(), ids.end(), target) != ids.end();
        state->repl_note_state->process(selected ? target : (ids.empty() ? -1 : ids.front()),
            static_cast<const uint32_t*>(context.eventIn().getMessages()),
            context.eventIn().position() / sizeof(uint32_t), context.frameCount());
        if (selected) repl_capture_scope(target, context);
    } else {
        state->repl_note_state.reset();
    }
    state->cached_audio_in_use.store(nullptr, std::memory_order_release);''')


def generate(checkout, source, output):
    includes = checkout / 'source/uapmd-engine/include/uapmd-engine'
    root = Path(__file__).resolve().parents[3]
    header = includes / 'detail/sequencer/FrozenTrackManager.hpp'
    text = header.read_text(encoding='utf-8')
    text = once(text, '    struct PlaybackState {',
                '    struct PlaybackState {\n        std::optional<repl_notes::InputState> repl_note_state;')
    text = once(text, '    struct PlaybackSnapshot {', '''    struct PlaybackSnapshot {
        std::vector<PlaybackState*> repl_retired;
        mutable bool repl_retired_cleared{};''')
    text = f'#include "{(root / "patches/uapmd/note-monitor/note_monitor.hpp").as_posix()}"\n' + text
    text = f'#include "{(root / "uapmd-host/scope_capture.hpp").as_posix()}"\n' + text
    text = re.sub(r'#include "([^"]+)"',
                  lambda m: m[0] if Path(m[1]).is_absolute() else f'#include "{(header.parent / m[1]).as_posix()}"', text)
    umbrella = (includes / 'uapmd-engine.hpp').read_text(encoding='utf-8')
    umbrella = once(umbrella, '#include "detail/sequencer/FrozenTrackManager.hpp"',
                    f'#include "{(output / "FrozenTrackManager.hpp").as_posix()}"')
    umbrella = re.sub(r'#include "(detail/[^"]+)"',
                      lambda m: f'#include "{(includes / m[1]).as_posix()}"', umbrella)
    files = {output / 'FrozenTrackManager.cpp': transform(source.read_text(encoding='utf-8')),
             output / 'FrozenTrackManager.hpp': text,
             output / 'include/uapmd-engine/uapmd-engine.hpp': umbrella}
    for path, content in files.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists() or path.read_text(encoding='utf-8') != content:
            path.write_text(content, encoding='utf-8')


if __name__ == '__main__':
    checkout, source, output = map(lambda value: Path(value).resolve(), sys.argv[1:])
    try:
        generate(checkout, source, output)
    except (OSError, ValueError) as error:
        revision = subprocess.run(['git', '-C', str(checkout), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'{NAME}: {source}: {error}; upstream {revision} (validated {REVISION}); '
                 f'see {Path(__file__).with_name("README.md")}')
