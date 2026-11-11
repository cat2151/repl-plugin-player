"""Installed-Surge preset checks, shared with the existing editor/API harness."""
from pathlib import Path
import struct
import subprocess
import sys
import time
import xml.etree.ElementTree as XML

ROOT = Path(__file__).resolve().parents[3]
# Test-owned topology and preset choice; built-in snippets may change freely.
SETUP = """var presetTrack = uapmd.sequencer.getTrackInfos().find(t => t.nodes.some(n => n.instanceId === surgeId)).trackIndex;
if (uapmd.timeline.createEmptyMidiClip(presetTrack, 0, 480, 120) < 0) throw new Error('clip creation failed');
if (uapmd.instancing.create('CLAP', 'org.baconpaul.six-sines', -1) < 0) throw new Error('instrument creation failed');"""
PRESET = 'Attacky'


def patch(bytes_, factory=False):
    assert bytes_[:4] == b'sub3', 'Not an actual Surge state'
    length = struct.unpack_from('<I', bytes_, 4)[0]
    assert 0 < length <= len(bytes_) - 32, 'Invalid XML bounds'
    xml_bytes = bytes_[32:32 + length].rstrip(b'\0')
    if factory:
        # The installed legacy micro-computer FXP declares UTF-8 but stores its
        # name as Windows-1252 (0xb5). Decode that original factory metadata for
        # comparison; actual saved plugin state remains strictly parsed UTF-8.
        try:
            xml_bytes.decode('utf-8')
        except UnicodeDecodeError:
            xml_bytes = xml_bytes.decode('cp1252')
    xml = XML.fromstring(xml_bytes)
    name = xml.find('meta').get('name')
    parameters = {p.tag: float(p.get('value')) for p in xml.find('parameters') if p.get('value') is not None}
    assert parameters, 'No state parameters'
    return name, parameters, int(xml.get('revision'))


def expected_parameters(factory, before):
    # SurgePatch::load_xml preserves volume/fx bypass for old presets and maps
    # legacy polyphony=8 to DEFAULT_POLYLIMIT=16 (revision <= 15).
    # https://github.com/surge-synthesizer/surge/blob/main/src/common/SurgePatch.cpp
    expected = dict(factory[1])
    if factory[2] < 17:
        expected['volume'] = before[1]['volume']
    if 'fx_bypass' in expected:
        expected['fx_bypass'] = before[1]['fx_bypass']
    if factory[2] <= 15 and expected.get('polylimit') == 8:
        expected['polylimit'] = 16
    # Verified Surge 1.3.4: SurgeStorage::fxt_ensemble == 20 and
    # BBDEnsembleEffect::handleStreamingMismatches(revision <= 22) initializes
    # ens_output_filter (parameter 11) to defaultFilterCut=45.2326448f (6 kHz),
    # deactivated. Old FXPs streamed a dummy integer zero in this added slot.
    # https://github.com/surge-synthesizer/surge/blob/f7b97c682ade0b87da85ca5968b63d5c7c98e68d/src/common/dsp/effects/BBDEnsembleEffect.cpp
    if factory[2] <= 22:
        for key, value in factory[1].items():
            if key.startswith('fx') and key.endswith('_type') and value == 20:
                expected[key[:-5] + '_p11'] = 45.2326448
    return expected


def check(call, evaluate, pump, data):
    def state():
        call('request-state')
        end = time.monotonic() + 20
        while not call('poll')['stateReady']:
            assert time.monotonic() < end, 'State request timed out'
            pump()
        return bytes.fromhex(call('state')['hex'])

    def wait(job):
        end = time.monotonic() + 20
        while job['state'] not in ('completed', 'failed'):
            assert time.monotonic() < end, 'Preset mutation timed out'
            pump()
            job = evaluate(f"uapmd.mutations.getJob({int(job['jobId'])})")
        return job

    presets = evaluate('uapmd.instance(surgeId).getPresets()')
    assert len(presets) > 600, 'Factory discovery did not finish'
    assert not any('\ufffd' in p['name'] for p in presets), 'Invalid UTF-8 metadata escaped'
    for expected in ('µcomputer', 'Sunrise Breakfast at the Digital Café', 'Wavetextures™ Pro Kit'):
        assert any(p['name'].casefold() == expected.casefold() for p in presets), expected
    # All real FXPs with an extension are represented, including provider-XML failures.
    files = [p for folder in ('patches_factory', 'patches_3rdparty')
             for p in (Path('C:/ProgramData/Surge XT') / folder).rglob('*')
             if p.is_file() and p.suffix == '.fxp']
    assert len(presets) >= len(files), (len(presets), len(files))
    assert len({p['index'] for p in presets}) == len(presets)
    evaluate(SETUP)
    topology = evaluate('uapmd.sequencer.getTrackInfos()')
    clips = evaluate('uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackIndex))')
    chosen = next(p for p in presets if p['name'] == PRESET)
    before = patch(state())
    result = evaluate(f"uapmd.instance(surgeId).loadPreset({int(chosen['index'])})")
    # Pending MutationJob serializes null safely, without unwinding through QuickJS.
    assert result['state'] in ('running', 'completed')
    job = wait(result)
    assert job['state'] == 'completed' and job['result']['success'], job
    loaded = patch(state())
    factory = patch(Path(f'C:/ProgramData/Surge XT/patches_factory/Basses/{PRESET}.fxp').read_bytes()[60:])
    assert loaded[0] == factory[0] == PRESET, loaded[0]
    assert loaded[1] != before[1], 'Accepted job did not change actual sound'
    expected = expected_parameters(factory, before)
    assert expected.keys() <= loaded[1].keys(), 'Missing original parameters'
    assert all(abs(loaded[1][k] - v) <= 1e-6 * max(1, abs(v)) for k, v in expected.items()), 'Factory parameter mismatch outside documented legacy conversion' 
    for invalid in (-1, 2147483647):
        failed = wait(evaluate(f'uapmd.instance(surgeId).loadPreset({invalid})'))
        assert failed['state'] == 'failed' and failed['error'], failed
        assert evaluate('1 + 1') == 2, 'Runtime failed after rejected load'
    # The UTF-8 load boundary differs from the legacy provider boundary.
    for name in ('µcomputer', 'Sunrise Breakfast at the Digital Café', 'Wavetextures™ Pro Kit'):
        selected = next(p for p in presets if p['name'].casefold() == name.casefold())
        job = wait(evaluate(f"uapmd.instance(surgeId).loadPreset({int(selected['index'])})"))
        assert job['state'] == 'completed' and job['result']['success'], job
        assert patch(state())[0].casefold() == name.casefold(), name
    assert evaluate('uapmd.sequencer.getTrackInfos()') == topology, 'Preset changed track/node topology'
    assert evaluate('uapmd.sequencer.getTrackInfos().map(t => uapmd.timeline.getClips(t.trackIndex))') == clips
    assert evaluate('uapmd.audio.isEngineEnabled()') is False
    return f'PASS presets: {len(presets)} entries, Unicode names/loads, factory state/parameters, job errors/recovery, loads preserve topology'


if __name__ == '__main__':
    raise SystemExit(subprocess.run([sys.executable, str(ROOT / 'scripts/plugin-ui-test.py')]).returncode)
