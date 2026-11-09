"""Offline Linux regression tests for GitHub installation and self-update."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import pty
import select
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/debug/cppm'


class UpdateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='cppm update (spaces & symbols) ')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tools = self.root / 'tools'
        self.tools.mkdir()
        self.downloads = self.root / 'downloads'
        self.downloads.mkdir()
        self.install = self.root / 'install bin'
        self.install.mkdir()
        self.binary = self.install / 'cppm'
        shutil.copy2(BINARY, self.binary)
        self.log = self.root / 'requests.log'
        self.env = os.environ.copy()
        for name in ('CI', 'CPPM_NO_UPDATE_CHECK', 'CPPM_OFFLINE', 'CPPM_SCRIPT_STACK'):
            self.env.pop(name, None)
        self.env.update({
            'PATH': str(self.tools) + os.pathsep + self.env['PATH'],
            'HOME': str(self.root),
            'XDG_CACHE_HOME': str(self.root / 'cache'),
            'CPPM_TEST_DOWNLOADS': str(self.downloads),
            'CPPM_TEST_REQUEST_LOG': str(self.log),
        })
        curl = self.tools / 'curl'
        curl.write_text(f'#!{sys.executable}\n' + '''
import os
from pathlib import Path
import sys
import time
from urllib.parse import urlparse

args = sys.argv[1:]
url = args[-1]
assert url.startswith('https://github.com/GaussNoob/cppm/releases/')
assert '--proto' in args and args[args.index('--proto') + 1] == '=https'
with open(os.environ['CPPM_TEST_REQUEST_LOG'], 'a') as log:
    log.write(url + '\\n')
if os.environ.get('CPPM_TEST_DELAY'):
    time.sleep(float(os.environ['CPPM_TEST_DELAY']))
source = Path(os.environ['CPPM_TEST_DOWNLOADS']) / Path(urlparse(url).path).name
if not source.is_file():
    sys.exit(22)
data = source.read_bytes()
if '--output' in args:
    Path(args[args.index('--output') + 1]).write_bytes(data)
else:
    sys.stdout.buffer.write(data)
''')
        curl.chmod(0o755)
        self.cache = self.root / 'cache/cppm/update.json'

    def run_cli(self, *args, **kwargs):
        return subprocess.run([self.binary, *args], cwd=self.root, env=self.env,
                              capture_output=True, text=True, timeout=15, **kwargs)

    def release(self, version='999.0.0'):
        (self.downloads / 'VERSION').write_text(version + '\n')
        arch = 'arm64' if platform.machine() in ('aarch64', 'arm64') else 'x86_64'
        stem = f'cppm-{version}-linux-{arch}'
        package = self.root / stem
        package.mkdir()
        binary = package / 'cppm'
        binary.write_text(f'#!/bin/sh\nprintf "cppm {version}\\n"\n')
        binary.chmod(0o755)
        shutil.copy2(ROOT / 'scripts/install.sh', package / 'install.sh')
        archive = self.downloads / f'{stem}.tar.gz'
        with tarfile.open(archive, 'w:gz') as output:
            output.add(package, arcname=stem)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_name(archive.name + '.sha256').write_text(f'{digest}  {archive.name}\n')
        return archive

    def bootstrap(self, *args):
        return subprocess.run(['sh', ROOT / 'scripts/bootstrap.sh', '--bin-dir', self.install, *args],
                              cwd=self.root, env=self.env, capture_output=True, text=True, timeout=15)

    def terminal(self, *args):
        master, slave = pty.openpty()
        try:
            process = subprocess.Popen([self.binary, *args], cwd=self.root, env=self.env,
                                       stdin=slave, stdout=slave, stderr=slave, close_fds=True)
            os.close(slave)
            slave = None
            output = b''
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                ready, _, _ = select.select([master], [], [], 0.1)
                if ready:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError:
                        break
                    if not chunk:
                        break
                    output += chunk
                elif process.poll() is not None:
                    break
            process.wait(timeout=5)
            self.assertEqual(process.returncode, 0, output.decode(errors='replace'))
            return output.decode(errors='replace')
        finally:
            os.close(master)
            if slave is not None:
                os.close(slave)

    def test_remote_install_and_pinned_version(self):
        self.release()
        result = self.bootstrap('--version', 'v999.0.0')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.run_cli('--version').stdout.strip(), 'cppm 999.0.0')
        self.assertNotIn('/VERSION', self.log.read_text())

    def test_checksum_failure_preserves_installed_binary(self):
        archive = self.release()
        archive.write_bytes(archive.read_bytes() + b'corruption')
        previous = self.binary.read_bytes()
        result = self.bootstrap()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('SHA-256 mismatch', result.stderr)
        self.assertEqual(self.binary.read_bytes(), previous)

    def test_invalid_version_cannot_reach_download_or_shell(self):
        for version in ('1.2.3;touch pwned', '1.2.3\n../../pwned'):
            result = self.bootstrap('--version', version)
            self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.log.exists())
        self.assertFalse((self.root / 'pwned').exists())

    def test_check_then_update_replaces_the_running_binary_in_custom_directory(self):
        self.release()
        previous = self.binary.read_bytes()
        result = self.run_cli('update', '--check')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('999.0.0', result.stdout)
        self.assertEqual(self.binary.read_bytes(), previous)
        result = self.run_cli('update')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.run_cli('--version').stdout.strip(), 'cppm 999.0.0')

    def test_no_downgrade_and_offline_makes_no_requests(self):
        (self.downloads / 'VERSION').write_text('0.0.1\n')
        previous = self.binary.read_bytes()
        result = self.run_cli('update')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.binary.read_bytes(), previous)
        self.log.unlink()
        result = self.run_cli('--offline', 'update', '--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.log.exists())

    def test_background_refresh_is_nonblocking_and_notice_uses_cache(self):
        (self.downloads / 'VERSION').write_text('999.0.0\n')
        self.env['CPPM_TEST_DELAY'] = '2'
        started = time.monotonic()
        self.terminal('packages')
        self.assertLess(time.monotonic() - started, 1.5)
        deadline = time.monotonic() + 5
        while not self.cache.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertEqual(json.loads(self.cache.read_text())['latest'], '999.0.0')
        output = self.terminal('packages')
        self.assertIn('cppm update', output)
        self.assertIn('999.0.0', output)
        self.assertEqual(len(self.log.read_text().splitlines()), 1)
        # Redirected output and offline use do not print notices or refresh.
        self.assertNotIn('Atualização', self.run_cli('packages').stderr)
        self.assertNotIn('Atualização', self.terminal('--offline', 'packages'))
        self.env['CPPM_NO_UPDATE_CHECK'] = '1'
        self.assertNotIn('Atualização', self.terminal('packages'))
        self.env.pop('CPPM_NO_UPDATE_CHECK')
        self.env['CI'] = 'true'
        self.assertNotIn('Atualização', self.terminal('packages'))
        self.assertEqual(len(self.log.read_text().splitlines()), 1)

    def test_network_failure_is_silent_and_preserves_known_update(self):
        self.cache.parent.mkdir(parents=True)
        self.cache.write_text(json.dumps({'checked_at': 1, 'latest': '999.0.0'}))
        result = self.run_cli('__update-check')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout + result.stderr, '')
        cache = json.loads(self.cache.read_text())
        self.assertEqual(cache['latest'], '999.0.0')
        self.assertTrue(cache['failed'])
        self.run_cli('__update-check')
        self.assertEqual(len(self.log.read_text().splitlines()), 1)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=BINARY)
    args, remaining = parser.parse_known_args()
    BINARY = args.binary.resolve()
    unittest.main(argv=[sys.argv[0], *remaining], verbosity=2)
