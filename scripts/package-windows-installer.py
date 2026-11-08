"""Build the Windows x64 setup .exe using NSIS (works on Linux and Windows)."""

import argparse
import hashlib
from pathlib import Path
import os
import subprocess
import tomllib


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=root / 'target/release/cppm.exe')
    parser.add_argument('--makensis', default='makensis')
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file() or binary.read_bytes()[:2] != b'MZ':
        parser.error('Build a Windows release executable before packaging setup.')
    version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
    dist = root / 'dist'
    dist.mkdir(exist_ok=True)
    output = dist / f'cppm-{version}-windows-x86_64-setup.exe'
    prefix = '/D' if os.name == 'nt' else '-D'
    subprocess.run([
        args.makensis,
        f'{prefix}VERSION={version}',
        f'{prefix}PROJECT_ROOT={root}',
        f'{prefix}BINARY_PATH={binary}',
        f'{prefix}OUTPUT_PATH={output}',
        str(root / 'scripts/windows/installer.nsi'),
    ], check=True)
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_name(output.name + '.sha256').write_text(f'{digest}  {output.name}\n')
    print(output)


if __name__ == '__main__':
    main()
