"""Run Windows unit/CLI tests under Wine; native MSVC and PowerShell stay in CI."""

import argparse
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wine', default='wine')
    parser.add_argument('--cargo', default='cargo')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    build = subprocess.run([
        args.cargo, 'test', '--locked', '--target', 'x86_64-pc-windows-gnu',
        '--no-run', '--message-format=json',
    ], cwd=root, text=True, stdout=subprocess.PIPE, check=True)
    binaries = []
    for line in build.stdout.splitlines():
        artifact = json.loads(line)
        if artifact.get('reason') != 'compiler-artifact' or not artifact.get('executable'):
            continue
        target = artifact['target']
        if target['kind'] == ['lib'] or (target['kind'] == ['test'] and target['name'] == 'cli'):
            binaries.append(artifact['executable'])
    if len(binaries) != 2:
        raise SystemExit(f'Expected unit and CLI test binaries, received {binaries}')
    environment = dict(os.environ)
    environment.setdefault('WINEPREFIX', str(root / '.qa' / 'wine-prefix'))
    environment.setdefault('WINEDEBUG', '-all')
    # These tests require real Windows CMake/MSVC or Windows PowerShell.
    native_only = [
        'native_project_builds_runs_tests_and_preserves_exit_status',
        'portable_scripts_preserve_arguments_and_inherit_options',
        'powershell_scripts_preserve_data_and_exit_status',
    ]
    for binary in binaries:
        command = [args.wine, binary, '--test-threads=1']
        for name in native_only:
            command.extend(['--skip', name])
        subprocess.run(command, cwd=root, env=environment, check=True)
    print('Windows GNU unit/CLI tests passed under Wine. Native MSVC, C++ builds and PowerShell need the Windows CI job.')


if __name__ == '__main__':
    main()
