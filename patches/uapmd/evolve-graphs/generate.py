"""Redirect registered evolve graphs through UAPMD's existing worker batch."""
from pathlib import Path
import sys

REVISION = '9f708d3300b4a6f8109eee0fadccf286db8b7a45'


def transform(text):
    changes = [
        ('0.8 * static_cast<double>(process.frameCount()) / timingSampleRate',
         'repl_evolve_graphs::workerBudget() * static_cast<double>(process.frameCount()) / timingSampleRate'),
        ('            const bool processGraph = !processedByExtension && !tracks_[i]->bypassed();',
         '            auto* processingGraph = repl_evolve_graphs::select(static_cast<int32_t>(i), &tracks_[i]->graph());\n'
         '            const bool processGraph = !processedByExtension && !tracks_[i]->bypassed();'),
        ('job = {processGraph ? &tracks_[i]->graph() : nullptr, &tp, 0, 0};',
         'job = {processGraph ? processingGraph : nullptr, &tp, 0, 0};'),
        ('                tracks_[i]->graph().processAudio(tp);',
         '                processingGraph->processAudio(tp);'),
    ]
    for old, new in changes:
        if text.count(old) != 1:
            raise ValueError(f'anchor must occur once ({text.count(old)}): {old}')
        text = text.replace(old, new)
    return '#include "' + str(Path(__file__).with_name('graphs.hpp')).replace('\\', '/') + '"\n' + text


def main():
    source, output = map(Path, sys.argv[1:3])
    revision = sys.argv[3] if len(sys.argv) > 3 else 'unknown'
    try:
        result = transform(source.read_text(encoding='utf-8'))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding='utf-8') != result:
            output.write_text(result, encoding='utf-8')
    except (OSError, ValueError) as error:
        raise SystemExit(f'evolve-graphs: {source}: {error}; upstream {revision}; '
                         f'see {Path(__file__).with_name("integration.md")}') from error


if __name__ == '__main__':
    main()
