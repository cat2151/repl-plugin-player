"""Measure ordinary tracks and evolve with fixed hashed states and four-note input.

Generated state, output observations and timing records stay outside the repository.
The diagnostic deliberately delays B beyond five seconds; product selection is untouched.
`--sounds 3` requests one plugin only (every switch reuses that plugin); the default
rotates through all fixed sounds. Each request is labelled `state` (evolve reloaded the
slot's own instance), `swap` (evolve attached an instance that existed before the
request), or `create` (an instance made for the request), and `cold`
for the first request of its plugin in the session. UI stalls come from the session's
frame monitor file (`<log>.frames.jsonl`).
"""
import argparse
from collections import Counter
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import time

ROOT = Path(__file__).resolve().parents[1]
APIS = {'ordinary': None, 'evolve': 'evolve'}


def cpu_seconds(child):
    times = [wintypes.FILETIME() for _ in range(4)]
    get_times = ctypes.WinDLL('kernel32', use_last_error=True).GetProcessTimes
    get_times.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    get_times.restype = wintypes.BOOL
    if not get_times(int(child._handle), *(ctypes.byref(t) for t in times)):
        raise ctypes.WinError(ctypes.get_last_error())
    return sum((t.dwHighDateTime << 32) | t.dwLowDateTime for t in times[2:]) / 10_000_000



def phase_timings(observations, evolve):
    groups = {}
    for observation in observations:
        slots = observation['slots']
        if not evolve:
            phase = 'ordinary'
        elif all(0 < slot['gain'] < 1 for slot in slots):
            phase = 'fade'
        elif any(slot['state'] in [0, 1, 4] for slot in slots):
            phase = 'preparation'
        else:
            phase = 'hold_or_prime'
        observation['phase'] = phase
        # A poll drains records since its predecessor; labels have one-poll uncertainty.
        for timing in observation['signal']['processingTimings']:
            if int(timing['stage']) != 5:
                continue
            group = groups.setdefault(phase, [])
            group.append(timing['durationMs'])
    return {key: dict(blocks=len(values), meanMs=sum(values)/len(values), maxMs=max(values))
            for key, values in groups.items()}


def ui_stalls(log, since, until):
    """Frame-monitor stalls overlapping [since, until] (wall seconds)."""
    path = log.with_suffix('.frames.jsonl')
    stalls = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.exists() else []
    return [s for s in stalls if s['endMs'] / 1000 >= since and s['startMs'] / 1000 <= until]


def stalled_ms(stalls, since, until):
    return sum(max(0, min(s['endMs'] / 1000, until) - max(s['startMs'] / 1000, since)) for s in stalls) * 1000


def request_summary(loads):
    groups = {}
    for load in loads:
        if 'settledWall' in load:
            groups.setdefault(f"{load['kind']}/{load['temperature']}", []).append(load)
    return {key: dict(count=len(group),
                      plugins=sorted({g['plugin'] for g in group}),
                      meanMs=sum(g['elapsedMs'] for g in group) / len(group),
                      maxMs=max(g['elapsedMs'] for g in group),
                      maxReturnedMs=max(g['returnedMs'] for g in group),
                      meanUiStallMs=sum(g['uiStallMs'] for g in group) / len(group),
                      maxUiStallMs=max(g['uiStallMs'] for g in group))
            for key, group in sorted(groups.items())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workers', type=int, nargs='+', default=[0, 6])
    parser.add_argument('--seconds', type=float, default=30)
    parser.add_argument('--fade-ms', type=float, default=1000)
    parser.add_argument('--topologies', nargs='+', choices=list(APIS), default=['ordinary', 'evolve'])
    parser.add_argument('--sounds', type=int, nargs='+', help='Indices into the fixed sounds to request')
    parser.add_argument('--fixture-directory', type=Path)
    parser.add_argument('--patch', nargs='+', default=[], metavar='PLUGIN=PATCH',
                        help='Catalog patch to fix instead of the first one; needs a new --fixture-directory')
    parser.add_argument('--require-clean', action='store_true',
                        help='Fail on discarded batches or deadline misses in evolve worker runs')
    args = parser.parse_args()
    directory = args.fixture_directory or Path(os.environ['LOCALAPPDATA']) / 'repl-plugin-player/investigations/evolve-fixed'
    directory.mkdir(parents=True, exist_ok=True)
    cells = dict(cell.split('\n', 1) for cell in
                 (ROOT / 'tests/e2e/evolve-fixed.js').read_text(encoding='utf-8').split('// %% ')[1:])
    Session = runpy.run_path(str(ROOT / 'scripts/cli-test.py'))['Session']
    manifest = directory / 'manifest.json'
    if not manifest.exists():
        session = Session(seconds=300)
        try:
            sounds = session.evaluate('(' + cells['catalog'] + ')()')
            for plugin, patch in (item.split('=', 1) for item in args.patch):
                next(s for s in sounds if s['plugin'] == plugin)['patch'] = patch
            for index, sound in enumerate(sounds):
                for key, cell, name in (('file', 'repl.patchState', 'state'), ('prepared', cells['prepared'], 'prepared')):
                    source = session.evaluate('(' + cell + ')(' + json.dumps(sound) + ')')
                    target = directory / f'{name}-{index}.bin'
                    shutil.copyfile(source, target)
                    sound.update({key: str(target), key + 'Sha256': hashlib.sha256(target.read_bytes()).hexdigest()})
            manifest.write_text(json.dumps(sounds, ensure_ascii=False, indent=2), encoding='utf-8')
            session.finish()
        finally:
            session.kill()
    sounds = json.loads(manifest.read_text(encoding='utf-8'))
    for plugin, patch in (item.split('=', 1) for item in args.patch):
        assert any(s['plugin'] == plugin and s['patch'] == patch for s in sounds), f'{manifest} has another {plugin}'
    for sound in sounds:
        for key in ('file', 'prepared'):
            assert hashlib.sha256(Path(sound[key]).read_bytes()).hexdigest() == sound[key + 'Sha256'], sound
    if args.sounds:
        sounds = [sounds[index] for index in args.sounds]
    results = []
    try:
        for topology in args.topologies:
            for workers in args.workers:
                session = Session(seconds=300)
                observations, loads = [], []
                result = dict(topology=topology, workers=workers, fadeMs=args.fade_ms,
                              sounds=[s['plugin'] for s in sounds], observations=observations, loads=loads)
                results.append(result)
                try:
                    api = APIS[topology]
                    evolve = api is not None
                    pending, first_seen, requested = {}, {}, set()
                    began = time.time()
                    session.evaluate('(' + cells['create'] + ')(' + json.dumps(dict(sounds=sounds,workers=workers,api=api,fadeMs=args.fade_ms)) + ')')

                    def absorb(observation):
                        """Settles drained outcomes; returns the slots settled now."""
                        now = time.time()
                        for instance in observation['instances']:
                            first_seen.setdefault(instance, now)
                        settled = set()
                        for outcome in observation['outcomes']:
                            request = next(r for r in loads if r.get('requestId') == outcome['requestId'])
                            assert not outcome['error'] and not outcome['stale'], outcome
                            after = observation['slots'][request['slot']]
                            before = request.pop('before')
                            if before['instance'] and before['instance'] == after['instance']:
                                kind = 'state'
                            elif first_seen.get(after['instanceId'], now) < request['submittedWall']:
                                kind = 'swap'
                            else:
                                kind = 'create'
                            request.update(settledWall=now, elapsedMs=outcome['elapsedMs'], kind=kind,
                                           instanceId=after['instanceId'])
                            del pending[request['slot']]
                            settled.add(request['slot'])
                        return settled

                    def load(slot, index, before):
                        sound = sounds[index % len(sounds)]
                        record = dict(slot=slot, index=index, plugin=sound['plugin'],
                                      temperature='warm' if sound['pluginId'] in requested else 'cold',
                                      submittedWall=time.time())
                        requested.add(sound['pluginId'])
                        start = time.perf_counter()
                        value = session.evaluate('(' + cells['load'] + ')(' + json.dumps(dict(slot=slot,sound=sound)) + ')')
                        record.update(returnedMs=1000*(time.perf_counter()-start), at=start)
                        record.update(requestId=value, before=dict(instance=before['instance']))
                        pending[slot] = record
                        loads.append(record)
                    if evolve:
                        observation = session.evaluate(cells['observe'])
                        absorb(observation)
                        load(0, 0, observation['slots'][0])
                        deadline = time.monotonic()+120
                        while observation['slots'][0]['state'] not in [2, 3]:
                            assert time.monotonic() < deadline, 'Initial A did not become ready'
                            time.sleep(.025)
                            observation = session.evaluate(cells['observe'])
                            absorb(observation)
                    session.evaluate(cells['start'])
                    before = session.evaluate(cells['observe'])
                    absorb(before)
                    start = time.perf_counter()
                    used_cpu = cpu_seconds(session.child)
                    sound_index = 1
                    end = start + (max(24, args.seconds) if evolve else min(8,args.seconds))
                    while time.perf_counter() < end:
                        observation = session.evaluate(cells['observe'])
                        elapsed = time.perf_counter()-start
                        observation['elapsed'] = elapsed
                        observations.append(observation)
                        settled = absorb(observation)
                        assert observation['signal']['finite'], observation
                        if evolve:
                            assert len(observation['slots']) == 2
                            for slot, state in enumerate(observation['slots']):
                                assert not state.get('error'), state
                                # First B load is artificially late, independent of preset speed.
                                if (state['state'] == 0 and slot not in pending and slot not in settled
                                        and (sound_index != 1 or elapsed >= 7)):
                                    load(slot, sound_index, state)
                                    sound_index += 1
                        time.sleep(.025)
                    # A new counter snapshot, so elapsed time and delivered blocks
                    # cover the same interval.
                    observation = session.evaluate(cells['observe'])
                    observation['elapsed'] = time.perf_counter()-start
                    observations.append(observation)
                    absorb(observation)
                    finished = time.time()
                    assert observation['signal']['finite'], observation
                    elapsed = observation['elapsed']
                    trace = session.evaluate(cells['trace'])
                    assert not trace['overflow'], trace
                    counts = Counter((e['instance'],e['note'],e['on']) for e in trace['events'])
                    assert counts and all(note in [60,64,67,71] for _,note,_ in counts), counts
                    # evolve reuses an instance for the same plugin, so it hears the chord again.
                    if not evolve:
                        assert all(count == 1 for (_,_,on),count in counts.items() if on), counts
                    if evolve:
                        assert len(loads) >= 4, loads
                        assert observations[0]['slots'][0]['instanceId'] >= 0
                        early = [o for o in observations if 2 < o['elapsed'] < 6]
                        assert early and max(o['signal']['peak'] for o in early) > .00001, 'Current A went silent while waiting for B'
                        assert all(o['slots'][0]['gain'] == 1 and o['slots'][1]['gain'] == 0 for o in early), 'Fade began before B was ready'
                        steady = [o for o in observations if o['elapsed'] >= 2]
                        assert all(o['signal']['peak'] > 1e-8 for o in steady), 'Observed a silent output window during sustained evolve playback'

                    assert max(o['signal']['peak'] for o in observations) > .00001, 'No actual output'
                    after = observations[-1]['signal']
                    phase_summary = phase_timings(observations,evolve)
                    if evolve:
                        assert 'fade' in phase_summary and 'preparation' in phase_summary, phase_summary
                    counters_before=before['diagnostics']['timingCounters']
                    counters_after=observations[-1]['diagnostics']['timingCounters']
                    worker_faults={o['diagnostics']['block'] for o in observations if o['diagnostics']['fault']}

                    result.update(phaseTimings=phase_summary,
                                  deadlineMisses=counters_after['deadlineMisses']-counters_before['deadlineMisses'],
                                  workerFaultBlocks=len(worker_faults),
                                  minPeak=min(o['signal']['peak'] for o in observations),
                                  seconds=elapsed, cpuPercentOfOneCore=100*(cpu_seconds(session.child)-used_cpu)/elapsed,
                                  peak=max(o['signal']['peak'] for o in observations), trace=trace,
                                  noteCounts={str(k):v for k,v in counts.items()},
                                  blocks=after['blocks']-before['signal']['blocks'])
                    callbacks = [t for o in observations for t in o['signal']['processingTimings']
                                 if int(t['stage']) == 5]
                    if callbacks and 'frames' in callbacks[0]:
                        formats = {(int(t['frames']), int(t['sampleRate'])) for t in callbacks}
                        assert len(formats) == 1, formats
                        frames, rate = next(iter(formats))
                        result.update(frames=frames, sampleRate=rate,
                                      deliveredFraction=result['blocks'] * frames / (elapsed * rate))
                    if args.require_clean and evolve and workers > 0:
                        assert result['workerFaultBlocks'] == 0, 'Worker discarded device blocks'
                        assert result['deadlineMisses'] == 0, 'Actual callback deadline overrun'
                        assert result.get('deliveredFraction', 0) > .98, 'Missing device-period processing'
                    session.evaluate(cells['cleanup'])
                    events=session.finish()
                    assert not any(e['event']=='periodic_script_error' for e in events), session.log
                    # The monitor writes a stall when it ends, so read after the session closed.
                    stalls = ui_stalls(session.log, began, finished)
                    for load_record in loads:
                        if 'settledWall' in load_record:
                            load_record['uiStallMs'] = stalled_ms(stalls, load_record['submittedWall'], load_record['settledWall'])
                    result.update(uiStalls=dict(count=len(stalls), maxMs=max((s['ms'] for s in stalls), default=0),
                                                totalMs=sum(s['ms'] for s in stalls),
                                                thresholdMs=sorted(s['thresholdMs'] for s in stalls)[len(stalls)//2] if stalls else None),
                                  stalls=stalls, requests=request_summary(loads))
                    print(json.dumps({k:v for k,v in result.items() if k not in ('observations','loads','trace','noteCounts','stalls')},ensure_ascii=False),flush=True)
                finally:
                    session.kill()
    finally:
        output=directory / f'ab-results-{time.time_ns()}.json'
        output.write_text(json.dumps(dict(sounds=sounds,results=results),ensure_ascii=False,indent=2),encoding='utf-8')
        print('Measurements:', output,flush=True)


if __name__ == '__main__':
    main()
