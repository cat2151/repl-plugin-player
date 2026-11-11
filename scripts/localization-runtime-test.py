"""Real host/native ja/en diagnostics with stable JSONL keys; silent normal Ready."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
EXE = ROOT / 'target/release/repl-plugin-player.exe'


def main():
    requests = [
        {'id': 1, 'command': 'eval', 'code': 'uapmd.instance(2147483647).showUI()'},
        {'id': 2, 'command': 'eval', 'code': "repl.text('no_such_translation')"},
        {'id': 3, 'command': 'eval', 'code': "repl.ui.show({id:'hidden',title:'Hidden',widgets:[]})"},
        {'id': 4, 'command': 'close'},
    ]
    for language, native, text, gui in [
            ('ja', 'プラグインインスタンスがありません', '不明な翻訳キー', '通常GUI専用'),
            ('en', 'Plugin instance not found', 'Unknown translation key', 'normal GUI')]:
        result = subprocess.run([str(EXE), '--lang', language, '--automation'], cwd=ROOT,
                                input=''.join(json.dumps(v) + '\n' for v in requests).encode(),
                                capture_output=True, timeout=180, creationflags=subprocess.CREATE_NO_WINDOW)
        events = [json.loads(line) for line in result.stdout.decode('utf-8').splitlines()]
        assert result.returncode != 0, events
        assert events[0]['event'] == 'ready', events
        closed = [event for event in events if event.get('event') == 'closed']
        assert len(closed) == 1 and closed[0]['ok'], events
        replies = {e['id']: e for e in events if 'id' in e}
        for identifier, expected in [(1, native), (2, text), (3, gui)]:
            assert replies[identifier]['ok'] is False and expected in replies[identifier]['error'], replies[identifier]
        assert replies[4]['ok'] is True
    print('PASS real ja/en native/Rust/script UI errors, unchanged JSONL keys, GUI-only API rejection, normal Ready and cleanup')


if __name__ == '__main__':
    main()
