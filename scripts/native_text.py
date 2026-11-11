"""Native-only test harness catalog callback; normal applications use Rust's host."""
import ctypes
import json
import re
import tomllib
from pathlib import Path


def register(dll, language='en'):
    catalog = tomllib.loads((Path(__file__).resolve().parents[1] / 'locales' / f'{language}.toml').read_text(encoding='utf-8'))
    reply = ctypes.create_string_buffer(1)

    @ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_char_p)
    def callback(request):
        nonlocal reply
        try:
            value = json.loads(request)
            assert value['api'] == 'text', value
            message = catalog[value['key']]
            if 'values' in value:
                arguments = value['values']
                assert isinstance(arguments, dict)
                def substitute(match):
                    item = arguments[match[1]]
                    assert isinstance(item, (str, int, float, bool))
                    return item if isinstance(item, str) else json.dumps(item)
                message = re.sub(r'\{\$(\w+)\}', substitute, message)
            response = {'ok': True, 'value': message}
        except Exception as error:
            response = {'ok': False, 'error': str(error)}
        reply = ctypes.create_string_buffer(json.dumps(response, ensure_ascii=False).encode())
        return ctypes.addressof(reply)

    dll.repl_set_script_api.argtypes = [ctypes.c_void_p]
    dll.repl_set_script_api(callback)
    return callback # Keep alive until after native session teardown.
