"""Package a native release binary with its per-user installer."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import tarfile
import tomllib
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', help='Rust target triple, for a binary built with cargo --target')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    targets = {
        'x86_64-pc-windows-msvc': ('windows', 'x86_64'),
        'x86_64-pc-windows-gnu': ('windows', 'x86_64'),
        'aarch64-pc-windows-msvc': ('windows', 'arm64'),
        'x86_64-unknown-linux-gnu': ('linux', 'x86_64'),
        'aarch64-unknown-linux-gnu': ('linux', 'arm64'),
    }
    if args.target:
        if args.target not in targets:
            parser.error(f'Unsupported release target: {args.target}')
        system, architecture = targets[args.target]
        binary_dir = root / 'target' / args.target / 'release'
    else:
        system = {'Windows': 'windows', 'Linux': 'linux'}.get(platform.system())
        architecture = {'AMD64': 'x86_64', 'aarch64': 'arm64'}.get(platform.machine(), platform.machine())
        binary_dir = root / 'target' / 'release'
    if system is None or architecture not in ('x86_64', 'arm64'):
        raise SystemExit('Packaging supports x86_64 and ARM64 on Linux and Windows.')
    version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
    binary = 'cppm.exe' if system == 'windows' else 'cppm'
    files = {
        binary: binary_dir / binary,
        'README.md': root / 'docs' / 'installation.md',
        'installation.md': root / 'docs' / 'installation.md',
        'guide.md': root / 'docs' / 'guide.md',
        'packages.md': root / 'docs' / 'packages.md',
        'architecture.md': root / 'docs' / 'architecture.md',
        'windows.md': root / 'docs' / 'windows.md',
        'LICENSE': root / 'LICENSE',
        'THIRD_PARTY_NOTICES.txt': root / 'THIRD_PARTY_NOTICES.txt',
    }
    installers = ['install.ps1', 'uninstall.ps1'] if system == 'windows' else ['install.sh']
    files.update({name: root / 'scripts' / name for name in installers})
    for source in files.values():
        if not source.is_file():
            raise SystemExit(f'Missing {source}; build with cargo build --release --locked first.')
    dist = root / 'dist'
    dist.mkdir(exist_ok=True)
    stem = f'cppm-{version}-{system}-{architecture}'
    metadata = dist / f'{stem}.release.json'
    metadata.write_text(json.dumps({
        'version': version,
        'platform': system,
        'architecture': architecture,
        'rust_target': args.target or f'{"aarch64" if architecture == "arm64" else architecture}-{"pc-windows-msvc" if system == "windows" else "unknown-linux-gnu"}',
    }, indent=2) + '\n', encoding='utf-8')
    files['release.json'] = metadata
    if system == 'windows':
        archive = dist / f'{stem}.zip'
        with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as package:
            for name, source in files.items():
                package.write(source, f'{stem}/{name}')
    else:
        archive = dist / f'{stem}.tar.gz'
        with tarfile.open(archive, 'w:gz') as package:
            for name, source in files.items():
                package.add(source, arcname=f'{stem}/{name}')
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    archive.with_name(archive.name + '.sha256').write_text(f'{digest}  {archive.name}\n')
    print(archive)


if __name__ == '__main__':
    main()
