"""Quiesce outgoing freeze work after validation, before plugin teardown."""
from pathlib import Path
import subprocess
import sys

REVISION = '4ec5c3b1c30130fdd3b7d4b375082273247e561f'
ANCHOR = '''    tearDownProjectInstances();

    // Delegate project loading to SequencerEngine asynchronously.'''
REPLACEMENT = '''    // Outgoing plugin removal invalidates frozen tracks. Disable their policy
    // before teardown so it cannot enqueue a new render against deleted plugins.
    // This runs only after parsing succeeds; invalid sources preserve the project.
    if (historyState().busy) {
        callback({false, "Wait for the pending undo operation before loading a project"});
        return;
    }
    auto& engine = *sequencer_.engine();
    auto& freeze = engine.frozenTrackManager();
    for (size_t i = 0; i < engine.tracks().size(); ++i)
        freeze.unfreezeTrack(static_cast<int32_t>(i));
    // A queued invalidation can mark a track Live while its renderer is active.
    // Drain cancellation and plugin restoration before destroying any instance.
    while (freeze.hasBusyTrack() || engine.offlineRendering()) {
        remidy::EventLoop::processQueuedTasks();
        std::this_thread::sleep_for(std::chrono::milliseconds(2));
    }
    tearDownProjectInstances();

    // Delegate project loading to SequencerEngine asynchronously.'''


def transform(source):
    if '    // Outgoing plugin removal invalidates frozen tracks.' in source:
        raise ValueError('repair already applied')
    if source.count(ANCHOR) != 1:
        raise ValueError(f'expected one load teardown anchor, found {source.count(ANCHOR)}')
    validation = source.index('    if (!UapmdProjectDataReader::read(projectFile)) {',
                              source.index('void uapmd_app::AppModel::loadProject('))
    if validation >= source.index(ANCHOR):
        raise ValueError('project validation no longer precedes teardown')
    return source.replace(ANCHOR, REPLACEMENT)


if __name__ == '__main__':
    source, output, checkout = map(Path, sys.argv[1:])
    try:
        content = transform(source.read_text(encoding='utf-8'))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding='utf-8') != content:
            output.write_text(content, encoding='utf-8')
    except (OSError, ValueError) as error:
        revision = subprocess.run(['git', '-C', str(checkout), 'rev-parse', 'HEAD'],
                                  capture_output=True, text=True).stdout.strip() or 'unknown'
        sys.exit(f'freeze-project-load: {source}: {error}; upstream {revision} '
                 f'(validated {REVISION}); see {Path(__file__).with_name("README.md")}')
