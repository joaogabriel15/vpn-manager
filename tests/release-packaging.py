#!/usr/bin/env python3
"""Check archive layout, permissions, integrity and incomplete-release rejection."""
import hashlib
import importlib.util
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def module(name, script):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / script)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


package = module('package', 'package-release.py')
verify = module('verify', 'verify-release.py').verify


class ReleaseTests(unittest.TestCase):
    def test_linux_layout_executable_permissions_checksum_and_no_clobber(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            binaries = base / 'bin'
            binaries.mkdir()
            for name in ['vpn-manager', 'vpn-manager-cli']:
                (binaries / name).write_bytes(b'synthetic executable')
            archive = package.build_archive('x86_64-unknown-linux-gnu', '0.2.0', binaries, base / 'dist', False)
            with tarfile.open(archive) as file:
                root = 'vpn-manager-0.2.0-linux-x86_64'
                self.assertEqual(file.getmember(root + '/bin/vpn-manager').mode & 0o777, 0o755)
                self.assertEqual(file.getmember(root + '/install.sh').mode & 0o777, 0o755)
                self.assertIn(root + '/README.md', file.getnames())
            with archive.open('rb') as file:
                checksum = hashlib.file_digest(file, 'sha256').hexdigest()
            self.assertEqual(archive.with_name(archive.name + '.sha256').read_text(), f'{checksum}  {archive.name}\n')
            with self.assertRaises(FileExistsError):
                package.build_archive('x86_64-unknown-linux-gnu', '0.2.0', binaries, base / 'dist', False)

    def test_windows_layout_and_missing_binary_rejection(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            binaries = base / 'bin'
            binaries.mkdir()
            (binaries / 'vpn-manager.exe').write_bytes(b'synthetic exe')
            with self.assertRaises(FileNotFoundError):
                package.build_archive('x86_64-pc-windows-msvc', '0.2.0', binaries, base / 'dist', False)
            (binaries / 'vpn-manager-native-cli.exe').write_bytes(b'synthetic cli')
            archive = package.build_archive('x86_64-pc-windows-msvc', '0.2.0', binaries, base / 'dist', False)
            with zipfile.ZipFile(archive) as file:
                self.assertIn('vpn-manager-0.2.0-windows-x86_64/vpn-manager-cli.exe', file.namelist())

    def test_publish_requires_all_targets_and_valid_checksums(self):
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            for target, ext in [('linux-x86_64', '.tar.gz'), ('windows-x86_64', '.zip'), ('macos-arm64', '.zip'), ('macos-x86_64', '.zip')]:
                name = f'vpn-manager-0.2.0-{target}{ext}'
                (folder / name).write_bytes(b'test artifact')
                digest = hashlib.sha256(b'test artifact').hexdigest()
                (folder / (name + '.sha256')).write_text(f'{digest}  {name}\n')
            verify(folder, '0.2.0')
            (folder / name).write_bytes(b'corrupted artifact')
            with self.assertRaises(ValueError):
                verify(folder, '0.2.0')
            (folder / name).unlink()
            with self.assertRaises(ValueError):
                verify(folder, '0.2.0')

    def test_rejects_version_path_traversal(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(ValueError):
                package.build_archive('x86_64-pc-windows-msvc', '../escape', Path(temp), Path(temp), False)


if __name__ == '__main__':
    unittest.main()
