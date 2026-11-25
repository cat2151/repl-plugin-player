"""Keep a bounded loop-head window resident without changing the dependency checkout."""
from pathlib import Path
import subprocess
import sys

NAME = 'frozen-loop-prefetch'
REVISION = '9f708d3300b4a6f8109eee0fadccf286db8b7a45'


def once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'expected one anchor, found {source.count(old)}: {old}')
    return source.replace(old, new)


def header(source):
    if 'loop_slots_' in source:
        raise ValueError('FrozenTrackAudio.hpp already has loop prefetch; inspect upstream support')
    source = once(source, '    bool loadAhead();', '''    bool loadAhead();
    // Audio thread publishes a cache-relative loop head; -1 disables prefetch.
    void playbackLoopStart(int64_t position) noexcept {
        loop_start_.store(position, std::memory_order_release);
    }''')
    return once(source, '    std::atomic<int64_t> play_position_{0};', '''    std::atomic<int64_t> play_position_{0};
    std::atomic<int64_t> loop_start_{-1};
    // Separate bank: normal forward lookahead must never evict the loop head.
    std::unique_ptr<Slot[]> loop_slots_;''')


def audio(source):
    if 'loop_slots_' in source:
        raise ValueError('FrozenTrackAudio.cpp already has loop prefetch; inspect upstream support')
    source = once(source, '    loader_encoded_.reserve(largest_stored_chunk_);', '''    loop_slots_ = std::make_unique<Slot[]>(slot_count_);
    for (uint32_t slot = 0; slot < slot_count_; ++slot)
        loop_slots_[slot].samples.resize(static_cast<size_t>(channel_count_) * kChunkFrames, 0.0f);
    loader_encoded_.reserve(largest_stored_chunk_);''')
    source = once(source, '        if (!copied && waitForDisk) {', '''        if (!copied && loop_slots_[slot].chunk.load(std::memory_order_acquire) == chunk) {
            reading_slot_.store(static_cast<int32_t>(slot + slot_count_), std::memory_order_seq_cst);
            if (loop_slots_[slot].chunk.load(std::memory_order_seq_cst) == chunk) {
                std::memcpy(output + done,
                    loop_slots_[slot].samples.data() + static_cast<size_t>(channel) * kChunkFrames + offset,
                    count * sizeof(float));
                copied = true;
            }
            reading_slot_.store(-1, std::memory_order_release);
        }
        if (!copied && waitForDisk) {''')
    begin = source.index('bool FrozenTrackAudio::loadAhead() {')
    end = source.index('\nFrozenTrackAudio::Summary', begin)
    original = source[begin:end]
    # Use the same checked channel-shift aware window and one-chunk scheduling
    # for both banks; an empty forward window must not suppress head prefetch.
    body = original[original.index('    int64_t first, last;'):original.rindex('    return false;')]
    body = once(body, '    neededChunks(position, first, last);', '    neededChunks(position, first, last);')
    body = body.replace('slots_[slot]', 'bank[slot]')
    body = body.replace('static_cast<int32_t>(slot)', 'static_cast<int32_t>(slot + bankOffset)')
    body = ''.join('    ' + line if line.strip() else line for line in body.splitlines(keepends=True))
    source = once(source, original, '''bool FrozenTrackAudio::loadAhead() {
    if (slot_count_ == 0) return false;
    auto loadWindow = [&](int64_t position, Slot* bank, uint32_t bankOffset) {
''' + body + '''        return false;
    };
    if (loadWindow(play_position_.load(std::memory_order_acquire), slots_.get(), 0)) return true;
    const auto head = loop_start_.load(std::memory_order_acquire);
    return head >= 0 && loadWindow(head, loop_slots_.get(), slot_count_);
}
''')
    return source


def manager(source, output):
    source = once(source, '#include "FrozenTrackAudio.hpp"',
                  f'#include "{(output / "FrozenTrackAudio.hpp").as_posix()}"')
    return once(source, '    cachedAudio->playbackPosition(cacheFrame);', '''    cachedAudio->playbackPosition(cacheFrame);
    cachedAudio->playbackLoopStart(looping
        ? std::max<int64_t>(0, loop.start - cachedAudio->startSample()) : -1);''')


def generate(checkout, manager_input, output):
    upstream = checkout / 'source/uapmd-engine/src/sequencer'
    files = {'FrozenTrackAudio.hpp': header((upstream / 'FrozenTrackAudio.hpp').read_text(encoding='utf-8')),
             'FrozenTrackAudio.cpp': audio((upstream / 'FrozenTrackAudio.cpp').read_text(encoding='utf-8')),
             'FrozenTrackManager.cpp': manager(manager_input.read_text(encoding='utf-8'), output)}
    for name, content in files.items():
        path = output / name
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists() or path.read_text(encoding='utf-8') != content:
            path.write_text(content, encoding='utf-8')


if __name__ == '__main__':
    checkout, manager_input, output = [Path(p).resolve() for p in sys.argv[1:]]
    try:
        generate(checkout, manager_input, output)
    except (OSError, ValueError) as error:
        revision = subprocess.run(['git', '-C', str(checkout), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'{NAME}: {checkout}, {manager_input}: {error}; upstream {revision} '
                 f'(validated {REVISION}); see {Path(__file__).with_name("README.md")}')
