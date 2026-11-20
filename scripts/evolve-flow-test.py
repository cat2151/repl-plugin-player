"""Count audio deadline misses per evolve plugin while a built-in snippet plays.

    python -B scripts/evolve-flow-test.py --start try-evolving-sequencer --end try-end-evolve
        --seconds 180 [--focus six-sines tyrelln6]

Runs the snippet as the user would (its sound choice is random), then polls the audio
worker diagnostics and `evolve.status` every `--interval` seconds. Each poll's increase of
`deadlineMisses` is charged to the plugins of the slots (pads and scatter) with gain > 0
(`sounding`) and to those loaded with gain 0 (`silent`; still processed), and to `focus`
when any slot's plugin id contains a `--focus` text. A miss is charged once per plugin even
when several slots hold it.
`patchSeconds`/`patchMisses` do the same for the sounding slots' "pluginId / patch".
`unfinished` counts, per worker fault block, the plugins whose track had not finished.
UI stalls come from the session's frame monitor file, as in evolve-fixed-test.py.
Wall times are kept, so polls overlapping `scripts/build-load.py`'s `busy` can be dropped.

`switches` lists each sound change of the buffers of every evolve group (`pad0`, `pad1`,
`scatter0`, `scatter1`) with the UI stalls overlapping its span, widened by `SLACK` seconds
on both sides. A buffer's sound is chosen when its request starts and the instance is made
later on the main thread, so its span runs until the first poll whose slot is READY or
ENGAGED. A switch still unsettled at the last poll has `settled` false, and its span ends
there, so it can take in the stall of `--end`. Stalls in no span are listed in `unmatchedStalls`.
Use `--interval 0.2` for switch spans of about one poll.
"""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import runpy
import time

ROOT = Path(__file__).resolve().parents[1]
OBSERVE = """(() => {
    let slots = [];
    try { slots = __replCall('evolve.status', {}); } catch (error) {}
    const pool = globalThis.evolvePool;
    // Indexed by slot, as `s`: the pads' group, then the scatter's.
    const buffers = pool ? pool.groups.flatMap((g, group) => g.buffers.map(b => ({b, who: group ? 'scatter' : 'pad'}))) : [];
    return {d: __remidy_audio_worker_diagnostics(), s: slots,
            who: buffers.map(({b, who}) => who + (b.slot - buffers.findIndex(x => x.who === who))),
            tracks: buffers.map(({b}) => b.track),
            patches: buffers.map(({b}) => b.sound ? b.sound.patch : null),
            sounds: buffers.map(({b}) => b.sound ? b.sound.plugin + ' / ' + b.sound.patch : null)};
})()"""
SLACK = .05
READY, ENGAGED = 2, 3


def stall_view(stall):
    return dict(startMs=stall['startMs'], ms=stall['ms'],
                functions=[f[0] if isinstance(f, list) else f for f in stall.get('functions', [])[:8]])


def switches(observations, stalls):
    """Sound changes with the stalls overlapping their spans; see the module text."""
    found = []
    for index, (previous, current) in enumerate(zip(observations, observations[1:]), 1):
        for slot, sound in enumerate(current.get('sounds', [])):
            before = previous['sounds'][slot] if slot < len(previous.get('sounds', [])) else None
            if not sound or sound == before:
                continue
            settled = next((o for o in observations[index:]
                            if slot < len(o['s']) and o['s'][slot]['state'] in (READY, ENGAGED)), None)
            found.append(dict(who=current['who'][slot], sound=sound, before=before, settled=settled is not None,
                              since=previous['wallBefore'], until=(settled or observations[-1])['wallAfter']))
    matched = set()
    for switch in found:
        overlapping = [i for i, s in enumerate(stalls)
                       if s['endMs'] / 1000 >= switch['since'] - SLACK and s['startMs'] / 1000 <= switch['until'] + SLACK]
        matched.update(overlapping)
        switch['stalls'] = [stall_view(stalls[i]) for i in overlapping]
        switch['maxStallMs'] = max((stalls[i]['ms'] for i in overlapping), default=0)
    unmatched = [stall_view(s) for i, s in enumerate(stalls) if i not in matched]
    return found, unmatched


def plugins_of(slots, sounding):
    return {s['pluginId'] for s in slots if s['pluginId'] and (s['gain'] > 0) == sounding}


def patches_of(observation, sounding):
    patches = observation.get('patches', [])
    return {f"{s['pluginId']} / {patches[i]}" for i, s in enumerate(observation['s'])
            if s['pluginId'] and i < len(patches) and (s['gain'] > 0) == sounding}


def summarize(observations, focus):
    sounding_seconds, silent_seconds = Counter(), Counter()
    sounding_misses, silent_misses, unfinished = Counter(), Counter(), Counter()
    patch_seconds, patch_misses = Counter(), Counter()
    focus_seconds = focus_misses = other_misses = 0
    faults = set()
    for previous, current in zip(observations, observations[1:]):
        dt = current['elapsed'] - previous['elapsed']
        misses = (current['d']['timingCounters']['deadlineMisses']
                  - previous['d']['timingCounters']['deadlineMisses'])
        # The interval belongs to the slots as they were at its start and end.
        slots = previous['s'] + current['s']
        sounding, silent = plugins_of(slots, True), plugins_of(slots, False) - plugins_of(slots, True)
        for plugin in plugins_of(previous['s'], True):
            sounding_seconds[plugin] += dt
        for plugin in plugins_of(previous['s'], False) - plugins_of(previous['s'], True):
            silent_seconds[plugin] += dt
        for plugin in sounding:
            sounding_misses[plugin] += misses
        for plugin in silent:
            silent_misses[plugin] += misses
        for patch in patches_of(previous, True):
            patch_seconds[patch] += dt
        for patch in patches_of(previous, True) | patches_of(current, True):
            patch_misses[patch] += misses
        focused = any(text.lower() in s['pluginId'].lower() for s in slots for text in focus)
        if focused:
            focus_seconds += dt
            focus_misses += misses
        else:
            other_misses += misses
        d = current['d']
        if d['fault'] and d['block'] not in faults:
            faults.add(d['block'])
            for track in d['atFault']['tracks']:
                if track['finishedNs'] == 0 and track['track'] in current['tracks']:
                    slot = current['s'][current['tracks'].index(track['track'])] if current['s'] else None
                    unfinished[slot['pluginId'] if slot else 'unknown'] += 1
    first, last = observations[0]['d']['timingCounters'], observations[-1]['d']['timingCounters']
    return dict(deadlineMisses=last['deadlineMisses'] - first['deadlineMisses'],
                workerFaultBlocks=len(faults),
                soundingSeconds=dict(sounding_seconds), silentSeconds=dict(silent_seconds),
                soundingMisses=dict(sounding_misses), silentMisses=dict(silent_misses),
                focusSeconds=focus_seconds if focus else None,
                focusMisses=focus_misses if focus else None,
                otherMisses=other_misses if focus else None,
                unfinished=dict(unfinished),
                patchSeconds=dict(patch_seconds), patchMisses=dict(patch_misses))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--start', required=True, help='Built-in snippet that starts evolve playback')
    parser.add_argument('--end', required=True, help='Built-in snippet that ends it')
    parser.add_argument('--seconds', type=float, default=180)
    parser.add_argument('--interval', type=float, default=.5)
    parser.add_argument('--focus', nargs='*', default=[], help='Plugin id texts counted together as focus')
    parser.add_argument('--output-directory', type=Path)
    args = parser.parse_args()
    directory = args.output_directory or Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/investigations/evolve-flow'
    directory.mkdir(parents=True, exist_ok=True)
    Session = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    fixed = runpy.run_path(str(ROOT / 'scripts/evolve-fixed-test.py'))
    observations = []
    result = dict(start=args.start, end=args.end, interval=args.interval, focus=args.focus,
                  observations=observations)
    session = Session(seconds=int(args.seconds) + 300)
    try:
        began = time.time()
        session.request('snippet', group='Built-in', name=args.start)
        start = time.perf_counter()
        used_cpu = fixed['cpu_seconds'](session.child)
        end = start + args.seconds
        while True:
            asked = time.time()
            observation = session.evaluate(OBSERVE)
            observation.update(elapsed=time.perf_counter() - start, wallMs=time.time() * 1000,
                               wallBefore=asked, wallAfter=time.time())
            observations.append(observation)
            if observation['elapsed'] >= args.seconds:
                break
            time.sleep(max(0, min(args.interval, end - time.perf_counter())))
        cpu = 100 * (fixed['cpu_seconds'](session.child) - used_cpu) / observations[-1]['elapsed']
        finished = time.time()
        session.request('snippet', group='Built-in', name=args.end)
        events = session.finish()
        assert not any(e['event'] == 'periodic_script_error' for e in events), session.log
        stalls = fixed['ui_stalls'](session.log, began, finished)
        changes, unmatched = switches(observations, stalls)
        result.update(switches=changes, unmatchedStalls=unmatched)
        result.update(summarize(observations, args.focus), seconds=observations[-1]['elapsed'],
                      cpuPercentOfOneCore=cpu, stalls=stalls,
                      uiStalls=dict(count=len(stalls), maxMs=max((s['ms'] for s in stalls), default=0),
                                    totalMs=sum(s['ms'] for s in stalls)))
        print(json.dumps({k: v for k, v in result.items()
                          if k not in ('observations', 'stalls', 'switches', 'unmatchedStalls')}), flush=True)
        for change in changes:
            mark = '' if change['settled'] else ' (unsettled)'
            print(f"{change['who']:8} {change['maxStallMs']:7.0f}ms {change['sound']}{mark}", flush=True)
        for stall in unmatched:
            print(f"unmatched {stall['ms']:7.0f}ms {stall['functions'][:3]}", flush=True)
    finally:
        session.kill()
        output = directory / f'flow-results-{time.time_ns()}.json'
        output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
        print('Measurements:', output, flush=True)


if __name__ == '__main__':
    main()
