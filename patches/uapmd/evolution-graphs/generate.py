"""Redirect registered evolve graphs through UAPMD's existing worker batch."""
from pathlib import Path
import sys
import re

REVISION = '4ec5c3b1c30130fdd3b7d4b375082273247e561f'


def transform(text, worker_header=None):
    changes = [
        ('0.8 * static_cast<double>(process.frameCount()) / timingSampleRate',
         'repl_evolution_graphs::workerBudget() * static_cast<double>(process.frameCount()) / timingSampleRate'),
        ('            const bool processGraph = !processedByExtension && !tracks_[i]->bypassed();',
         '            auto* processingGraph = repl_evolution_graphs::select(static_cast<int32_t>(i), &tracks_[i]->graph());\n'
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
    anchor = '        auto startTime = std::chrono::steady_clock::now();'
    if text.count(anchor) != 1:
        raise ValueError('coordinator priority poll anchor changed')
    text=text.replace(anchor,anchor+'\n        if (!offline_rendering_.load(std::memory_order_acquire)) repl_evolution_graphs::pollWorkers();')
    if worker_header is not None:
        text,count=re.subn(r'#include "[^"\n]*/AudioWorkers.hpp"',
                           lambda _: '#include "'+worker_header.as_posix()+'"',text)
        if count!=1:
            raise ValueError('private worker header must occur once')
    return '#include "' + str(Path(__file__).with_name('graphs.hpp')).replace('\\', '/') + '"\n' + text


def worker_outputs(directory):
    pool=(directory/'AudioTrackWorkerPool.hpp').read_text(encoding='utf-8')
    anchor='            const auto ticket = admission_.ticket();'
    if pool.count(anchor)!=1:
        raise ValueError('worker priority poll anchor changed')
    pool=pool.replace(anchor,'            repl_evolution_graphs::pollWorkers();\n'+anchor)
    pool='#include "'+Path(__file__).with_name('graphs.hpp').as_posix()+'"\n'+pool
    header=(directory/'AudioWorkers.hpp').read_text(encoding='utf-8')
    implementation=(directory/'AudioWorkers.cpp').read_text(encoding='utf-8')
    if header.count('#include "AudioTrackWorkerPool.hpp"')!=1:
        raise ValueError('AudioWorkers.hpp private pool include changed')
    if implementation.count('#include "AudioWorkers.hpp"')!=1:
        raise ValueError('AudioWorkers.cpp private header include changed')
    return {
        'AudioTrackWorkerPool.hpp':pool,
        'AudioWorkers.hpp':header,
        'AudioWorkers.cpp':implementation,
    }



def main():
    source, output = map(Path, sys.argv[1:3])
    revision = sys.argv[3] if len(sys.argv) > 3 else 'unknown'
    try:
        directory=Path(sys.argv[4])
        outputs=worker_outputs(directory)
        outputs[output.name]=transform(source.read_text(encoding='utf-8'),output.with_name('AudioWorkers.hpp'))
        output.parent.mkdir(parents=True, exist_ok=True)
        for name,result in outputs.items():
            target=output.with_name(name)
            if not target.exists() or target.read_text(encoding='utf-8') != result:
                target.write_text(result, encoding='utf-8')
    except (OSError, ValueError) as error:
        raise SystemExit(f'evolution-graphs: {source}: {error}; upstream {revision}; '
                         f'see {Path(__file__).with_name("integration.md")}') from error


if __name__ == '__main__':
    main()
