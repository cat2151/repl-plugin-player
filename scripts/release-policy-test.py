"""Exercise the workflow's actual skip shell and inspect publication gates."""
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[1]


def main():
    workflow = yaml.safe_load((ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8'))
    triggers = workflow.get('on', workflow.get(True))
    assert triggers['schedule'][0]['cron'] == '23 16 * * *'
    assert triggers['workflow_dispatch']['inputs']['publish']['default'] is False
    assert triggers['workflow_dispatch']['inputs']['publish']['type'] == 'boolean'
    jobs = workflow['jobs']
    assert jobs['publish']['needs'] == ['resolve', 'build']
    assert jobs['publish']['permissions'] == {'contents': 'write'}
    assert workflow['permissions'] == {'contents': 'read'}
    assert jobs['publish']['concurrency']['cancel-in-progress'] is False
    assert jobs['build']['if'] == "needs.resolve.outputs.build == 'true'"
    assert jobs['publish']['if'] == "github.event_name == 'schedule' || inputs.publish"
    assert 'always()' not in jobs['publish']['if'] # Failed build cannot publish/delete.
    source = next(s['run'] for s in jobs['resolve']['steps'] if s.get('id') == 'changed')
    actions = [s for job in jobs.values() for s in job['steps'] if 'uses' in s]
    siblings = {s.get('with', {}).get('repository') for s in actions}
    assert 'atsushieno/uapmd' in siblings and 'cat2151/clap-mml-render-tui' in siblings
    assert 'cat2151/clap-mml-play-server' not in siblings
    bash = str(Path(shutil.which('git')).resolve().parent.parent / 'bin/bash.exe') if os.name == 'nt' else shutil.which('bash')
    with tempfile.TemporaryDirectory(prefix='repl-release-policy-') as temporary:
        directory = Path(temporary)
        script = directory / 'decision.sh'
        script.write_text('gh() { printf "%s\\n" "$PUBLISHED"; }\n' + source, encoding='utf-8')
        for event, previous, current, expected in [
                ('schedule', '', 'new', True), ('schedule', '<!-- build-inputs same -->', '<!-- build-inputs same -->', False),
                ('schedule', '<!-- build-inputs old -->', '<!-- build-inputs new -->', True),
                ('workflow_dispatch', '<!-- build-inputs same -->', '<!-- build-inputs same -->', True)]:
            output = directory / 'output'
            output.write_text('')
            env = dict(os.environ, EVENT=event, PUBLISHED=previous, INPUTS=current, TAG='v0.1.0',
                       GITHUB_OUTPUT=output.as_posix(), GITHUB_STEP_SUMMARY=(directory / 'summary').as_posix())
            subprocess.run([bash, '--noprofile', '--norc', str(script)], env=env, check=True, capture_output=True)
            assert output.read_text().strip() == f'build={str(expected).lower()}'
    print('PASS release initial/unchanged/changed/manual decisions, failed-build publication gate, permissions and sibling layout')


if __name__ == '__main__':
    main()
