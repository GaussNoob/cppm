"""Validate a stable release tag and optionally write the public VERSION asset."""

import argparse
import os
from pathlib import Path
import re
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise SystemExit('Only stable X.Y.Z releases are supported.')
    tag = os.environ.get('RELEASE_TAG')
    if tag != f'v{version}':
        raise SystemExit(f'Release tag {tag!r} must match Cargo.toml: v{version}')
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(version + '\n', encoding='utf-8')
    print(f'Release version: {version}')


if __name__ == '__main__':
    main()
