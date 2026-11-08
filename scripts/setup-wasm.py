"""Install cppm's private Emscripten SDK without requiring Git or shell activation."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile


SDK_VERSION = '6.0.13'
SDK_COMMIT = '35659964bc2762fffec0d257997fb61475f765b7'
SOURCE_URL = f'https://github.com/emscripten-core/emsdk/archive/{SDK_COMMIT}.zip'
SOURCE_SHA256 = '1965569d3c03bf86d66d4abd6ffdfabca994301c8361ba9233f47454b16cf9d4'
MARKER = 'cppm.emsdk.json'


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def download_source(destination):
    if destination.is_file() and digest(destination) == SOURCE_SHA256:
        return
    partial = destination.with_suffix('.part')
    for attempt in range(3):
        try:
            request = urllib.request.Request(SOURCE_URL, headers={'User-Agent': 'cppm-sdk-setup'})
            with urllib.request.urlopen(request, timeout=60) as source, partial.open('wb') as target:
                shutil.copyfileobj(source, target)
            if digest(partial) != SOURCE_SHA256:
                raise RuntimeError('The Emscripten SDK source archive failed SHA-256 verification.')
            partial.replace(destination)
            return
        except Exception:
            partial.unlink(missing_ok=True)
            if attempt == 2:
                raise
            print('SDK source download failed; retrying...', flush=True)
            time.sleep(2)


def write_marker(directory, metadata):
    temporary = directory / (MARKER + '.tmp')
    temporary.write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    temporary.replace(directory / MARKER)


def prepare_source(directory):
    marker = directory / MARKER
    metadata = {'format': 1, 'managedBy': 'cppm'}
    if marker.is_file():
        metadata = json.loads(marker.read_text(encoding='utf-8-sig'))
        if metadata.get('managedBy') != 'cppm' or metadata.get('format') != 1:
            raise RuntimeError(f'Refusing to modify an SDK directory not owned by cppm: {directory}')
    elif directory.exists() and any(directory.iterdir()):
        raise RuntimeError(f'Refusing to modify a nonempty SDK directory without a cppm marker: {directory}')
    directory.mkdir(parents=True, exist_ok=True)
    write_marker(directory, metadata)
    if metadata.get('sourceCommit') == SDK_COMMIT and (directory / 'emsdk.py').is_file():
        return metadata
    downloads = directory / '.cppm-downloads'
    downloads.mkdir(exist_ok=True)
    archive = downloads / f'emsdk-{SDK_COMMIT}.zip'
    download_source(archive)
    with tempfile.TemporaryDirectory(prefix='cppm-sdk-source-', dir=directory) as temporary:
        stage = Path(temporary)
        prefix = f'emsdk-{SDK_COMMIT}'
        with zipfile.ZipFile(archive) as source:
            for item in source.infolist():
                path = PurePosixPath(item.filename)
                if not path.parts or path.parts[0] != prefix or '..' in path.parts or path.is_absolute():
                    raise RuntimeError('Unexpected path in the verified SDK source archive.')
                if (item.external_attr >> 16) & 0o170000 == 0o120000:
                    raise RuntimeError('Unexpected symbolic link in the SDK source archive.')
            source.extractall(stage)
        source = stage / prefix
        if not (source / 'emsdk.py').is_file():
            raise RuntimeError('emsdk.py is missing from the SDK source archive.')
        shutil.copytree(source, directory, dirs_exist_ok=True)
    metadata['sourceCommit'] = SDK_COMMIT
    write_marker(directory, metadata)
    return metadata


def sdk_environment(directory):
    environment = os.environ.copy()
    environment.update(
        EMSDK=str(directory),
        EM_CONFIG=str(directory / '.emscripten'),
        CFGDIR=str(directory),
        EMSDK_PYTHON=sys.executable,
        PYTHONUTF8='1',
    )
    # An unrelated activated SDK/cache must not redirect this installation.
    for name in ('EM_CACHE', 'EM_LLVM_ROOT', 'EM_BINARYEN_ROOT', 'EM_NODE_JS', 'EMSDK_NODE'):
        environment.pop(name, None)
    return environment


def verify_sdk(directory, environment):
    compiler = directory / 'upstream/emscripten/em++.py'
    subprocess.run([sys.executable, str(compiler), '--version'], cwd=directory, env=environment, check=True)
    probe = r"""import json, os, runpy, sys
values = runpy.run_path(sys.argv[1])
node = values['NODE_JS']
if isinstance(node, (tuple, list)):
    node = node[0]
print(json.dumps(os.path.expandvars(str(node))))
"""
    node = json.loads(subprocess.check_output(
        [sys.executable, '-E', '-c', probe, str(directory / '.emscripten')],
        cwd=directory, env=environment, text=True,
    ))
    subprocess.run([node, '--version'], cwd=directory, env=environment, check=True)
    with tempfile.TemporaryDirectory(prefix='cppm-wasm-probe-') as temporary:
        root = Path(temporary)
        source, output = root / 'main.cpp', root / 'probe.js'
        source.write_text('#include <cstdio>\nint main() { std::puts("cppm-wasm-ready"); }\n', encoding='utf-8')
        subprocess.run([
            sys.executable, str(compiler), str(source), '-std=c++20',
            '-sENVIRONMENT=node', '-o', str(output),
        ], cwd=root, env=environment, check=True)
        if output.with_suffix('.wasm').read_bytes()[:4] != b'\0asm':
            raise RuntimeError('The SDK did not produce a valid WebAssembly module.')
        result = subprocess.check_output([node, str(output)], cwd=root, env=environment, text=True)
        if result.strip() != 'cppm-wasm-ready':
            raise RuntimeError('Node.js could not run the compiled WebAssembly probe.')


def install(directory):
    directory = directory.expanduser().resolve()
    print(f'Preparing Emscripten {SDK_VERSION} and its Node.js in {directory}', flush=True)
    metadata = prepare_source(directory)
    environment = sdk_environment(directory)
    driver = directory / 'emsdk.py'
    for action in ('install', 'activate'):
        subprocess.run([sys.executable, str(driver), action, SDK_VERSION], cwd=directory, env=environment, check=True)
    verify_sdk(directory, environment)
    metadata['sdkVersion'] = SDK_VERSION
    write_marker(directory, metadata)
    print('WebAssembly SDK ready: C++ compiled to WASM and executed with Node.js.', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    # This location is also recognized by cppm 0.4.1, allowing an existing
    # installation to be repaired without replacing the CLI or its projects.
    parser.add_argument('--sdk-dir', type=Path, default=Path.home() / '.local/share/cppm/emsdk')
    arguments = parser.parse_args()
    try:
        install(arguments.sdk_dir)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        parser.exit(1, f'WebAssembly setup failed: {error}\nRerun the same command to retry.\n')


if __name__ == '__main__':
    main()
