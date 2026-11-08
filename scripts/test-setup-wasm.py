"""Offline tests for SDK bootstrap ownership, integrity, retries and environment isolation."""

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile


spec = importlib.util.spec_from_file_location('sdk_setup', Path(__file__).with_name('setup-wasm.py'))
sdk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sdk)


class SetupTests(unittest.TestCase):
    def test_invalid_download_is_retried_and_not_installed(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / 'sdk.zip'
            with patch.object(sdk.urllib.request, 'urlopen', side_effect=lambda *a, **k: io.BytesIO(b'invalid')) as request:
                with patch.object(sdk.time, 'sleep'):
                    with self.assertRaisesRegex(RuntimeError, 'SHA-256'):
                        sdk.download_source(archive)
            self.assertEqual(request.call_count, 3)
            self.assertFalse(archive.exists())
            self.assertFalse(archive.with_suffix('.part').exists())

    def test_cached_verified_download_does_not_contact_the_network(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / 'sdk.zip'
            archive.write_bytes(b'verified')
            with patch.object(sdk, 'SOURCE_SHA256', hashlib.sha256(b'verified').hexdigest()):
                with patch.object(sdk.urllib.request, 'urlopen', side_effect=AssertionError('unexpected network')):
                    sdk.download_source(archive)

    def test_existing_user_directory_is_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            user_file = directory / 'keep.txt'
            user_file.write_text('user data')
            with self.assertRaisesRegex(RuntimeError, 'without a cppm marker'):
                sdk.prepare_source(directory)
            self.assertEqual(user_file.read_text(), 'user data')

    def test_verified_source_is_installed_once_and_records_ownership(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / 'sdk'
            archive = Path(temporary) / 'source.zip'
            with zipfile.ZipFile(archive, 'w') as source:
                source.writestr(f'emsdk-{sdk.SDK_COMMIT}/emsdk.py', '# official driver fixture')
            def copy_source(destination):
                destination.write_bytes(archive.read_bytes())
            with patch.object(sdk, 'download_source', side_effect=copy_source) as download:
                metadata = sdk.prepare_source(directory)
                sdk.prepare_source(directory)
            self.assertEqual(download.call_count, 1)
            self.assertEqual(metadata['sourceCommit'], sdk.SDK_COMMIT)
            self.assertEqual(json.loads((directory / sdk.MARKER).read_text())['managedBy'], 'cppm')
            self.assertTrue((directory / 'emsdk.py').is_file())

    def test_archive_cannot_escape_installation(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / 'sdk'
            archive = Path(temporary) / 'source.zip'
            with zipfile.ZipFile(archive, 'w') as source:
                source.writestr(f'emsdk-{sdk.SDK_COMMIT}/../../escape.txt', 'untrusted')
            with patch.object(sdk, 'download_source', side_effect=lambda target: target.write_bytes(archive.read_bytes())):
                with self.assertRaisesRegex(RuntimeError, 'Unexpected path'):
                    sdk.prepare_source(directory)
            self.assertFalse((Path(temporary) / 'escape.txt').exists())

    def test_install_uses_private_environment_without_git_or_global_activation(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            original = {'EMSDK': 'user-sdk', 'EM_CONFIG': 'user-config', 'EM_CACHE': 'user-cache', 'EMSDK_NODE': 'user-node'}
            with patch.dict(os.environ, original):
                with patch.object(sdk, 'prepare_source', return_value={'format': 1, 'managedBy': 'cppm'}):
                    with patch.object(sdk.subprocess, 'run') as run, patch.object(sdk, 'verify_sdk'), patch.object(sdk, 'configure_windows_tools'):
                        sdk.install(directory)
                    self.assertEqual(run.call_count, 2)
                    for call in run.call_args_list:
                        arguments, = call.args
                        self.assertEqual(arguments[0], sdk.sys.executable)
                        self.assertNotIn('--permanent', arguments)
                        self.assertNotIn('--system', arguments)
                        self.assertNotIn('EM_CACHE', call.kwargs['env'])
                        self.assertEqual(call.kwargs['env']['EM_CONFIG'], str(directory.resolve() / '.emscripten'))
                    for name, value in original.items():
                        self.assertEqual(os.environ[name], value)

    def test_windows_minifier_uses_node_with_literal_sdk_paths_and_survives_reinstall(self):
        with tempfile.TemporaryDirectory(prefix='sdk spaces & symbols ') as temporary:
            directory = Path(temporary)
            shared = directory / 'upstream/emscripten/tools/shared.py'
            shared.parent.mkdir(parents=True)
            shared.write_text("def get_npm_cmd(name, missing_ok=False):\n  return ['original.cmd']\n", encoding='utf-8')
            sdk.configure_windows_tools(directory)
            source = shared.read_text(encoding='utf-8')
            sdk.configure_windows_tools(directory)
            self.assertEqual(shared.read_text(encoding='utf-8'), source)
            node = 'C:/private node/node.exe'
            from types import SimpleNamespace
            context = {'utils': SimpleNamespace(WINDOWS=True), 'config': SimpleNamespace(NODE_JS=[node]), 'path_from_root': lambda path: str(directory / path)}
            exec(compile(source, str(shared), 'exec'), context)
            self.assertEqual(context['get_npm_cmd']('html-minifier-terser'), [node, str(directory / 'node_modules/html-minifier-terser/cli.js')])
            self.assertEqual(context['get_npm_cmd']('another-tool'), ['original.cmd'])


if __name__ == '__main__':
    unittest.main()
