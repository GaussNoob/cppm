import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
from urllib.request import urlopen


binary, directory, template, mode = sys.argv[1:]
project = Path(directory)
name = 'web-app' if template == 'wasm' else 'api-app'
with socket.socket() as reservation:
    reservation.bind(('127.0.0.1', 0))
    port = reservation.getsockname()[1]

with tempfile.TemporaryFile(mode='w+') as log:
    command = [binary, '-C', str(project), mode, 'dev']
    if mode == 'watch':
        command += ['--interval', '50', '--debounce', '150']
    server = subprocess.Popen(
        command + ['--', '--port', str(port)],
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=os.name == 'posix',
    )
    try:
        deadline = time.monotonic() + 120
        base = f'http://127.0.0.1:{port}'
        route = f'/{name}.html' if template == 'wasm' else '/health'
        while True:
            if server.poll() is not None:
                raise RuntimeError('cppm run dev exited before becoming ready')
            try:
                with urlopen(base + route, timeout=1) as response:
                    body = response.read()
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise RuntimeError('cppm run dev did not become ready')
                time.sleep(0.1)

        if template == 'wasm':
            assert b'C++ running in WebAssembly' in body, body
            with urlopen(base + f'/{name}.js', timeout=2) as response:
                assert response.read(), 'JavaScript artifact is empty'
            with urlopen(base + f'/{name}.wasm', timeout=2) as response:
                assert response.headers.get_content_type() == 'application/wasm'
                assert response.read()[:4] == b'\0asm', 'Invalid WebAssembly artifact'
        else:
            expected = {'status': 'ok', 'framework': 'drogon'}
            assert json.loads(body) == expected, body
            with urlopen(base + '/', timeout=2) as response:
                assert response.headers.get_content_type() == 'application/json'
                assert json.load(response) == expected
        if mode == 'watch':
            if template == 'wasm':
                with urlopen(base + f'/{name}.wasm', timeout=2) as response:
                    previous = response.read()
            source = project / 'src/main.cpp'
            text = source.read_text()
            source.write_text(text.replace('Hello from C++!', 'Hello after watch!') if template == 'wasm'
                              else text.replace('body["status"] = "ok"', 'body["status"] = "updated"'))
            deadline = time.monotonic() + 30
            while True:
                if server.poll() is not None:
                    raise RuntimeError('cppm watch exited during rebuild')
                try:
                    with urlopen(base + (f'/{name}.wasm' if template == 'wasm' else '/health'), timeout=1) as response:
                        updated = response.read()
                    if template == 'wasm' and updated != previous and updated[:4] == b'\0asm':
                        break
                    if template == 'drogon' and json.loads(updated) == {'status': 'updated', 'framework': 'drogon'}:
                        break
                except OSError:
                    pass
                if time.monotonic() >= deadline:
                    raise RuntimeError('cppm watch did not serve the rebuilt application')
                time.sleep(0.1)
            print(f'{template}: watch rebuilt and restarted on the same port')
        print(f'{template}: cppm run dev served the expected artifacts/routes on port {port}')
    finally:
        # cppm run dev starts another cppm process and then Python/the API.
        # Stop the entire process tree so the test leaves no server running.
        if os.name == 'posix':
            try:
                os.killpg(server.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        elif server.poll() is None:
            subprocess.run(['taskkill', '/PID', str(server.pid), '/T', '/F'], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        server.wait(timeout=10)
        log.seek(0)
        print(log.read())
        try:
            with socket.create_connection(('127.0.0.1', port), timeout=1):
                raise AssertionError('A development server remains running after shutdown')
        except OSError:
            pass
