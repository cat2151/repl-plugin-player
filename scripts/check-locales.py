"""Check catalog parity and source references, without launching native/audio code."""
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def check(directory=ROOT / 'locales'):
    catalogs = {lang: tomllib.loads((directory / f'{lang}.toml').read_text(encoding='utf-8'))
                for lang in ('ja', 'en')}
    assert catalogs['ja'].keys() == catalogs['en'].keys(), 'translation key mismatch'
    for key, value in catalogs['en'].items():
        assert isinstance(value, str) and isinstance(catalogs['ja'][key], str), key
        arguments = lambda text: set(re.findall(r'\{\$(\w+)\}', text))
        assert arguments(value) == arguments(catalogs['ja'][key]), f'placeholder mismatch: {key}'
    used = set()
    for directory in ('src', 'native', 'snippets/builtin', 'patches'):
        for path in (ROOT / directory).rglob('*'):
            if path.suffix not in ('.rs', '.js', '.hpp', '.cpp', '.py'):
                continue
            text = path.read_text(encoding='utf-8')
            used.update(re.findall(r'(?:i18n::(?:t|render)|\b(?:t|render)|repl_localized|repl_format1)\(\s*"([\w]+)"', text))
            used.update(re.findall(r"repl\.text\('([\w]+)'\)", text))
            used.update(re.findall(r'// (?:%% )?@text (\w+)', text))
    assert not used - catalogs['en'].keys(), f'missing catalog keys: {used - catalogs["en"].keys()}'
    print(f'PASS ja/en: {len(catalogs["en"])} keys; {len(used)} source references; matching placeholders')


if __name__ == '__main__':
    check()
