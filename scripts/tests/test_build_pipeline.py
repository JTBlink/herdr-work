"""Build reuse, conservative cleanup and failure-safe archive publication."""
import hashlib
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import build
import cleanup
import package

TARGET = 'aarch64-apple-darwin'
LINUX = 'x86_64-unknown-linux-gnu'


def aged_file(path, content=b'cache', days=10):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    when = time.time() - days * 86400
    os.utime(path, (when, when))
    return path


def age_tree(path):
    when = time.time() - 10 * 86400
    for entry in [*path.rglob('*'), path]:
        os.utime(entry, (when, when))


def archive_pair(root, version, target=TARGET, days=0):
    path = aged_file(root / f'herdr-work-{version}-{target}.tar.gz', version.encode(), days)
    checksum = hashlib.sha256(path.read_bytes()).hexdigest()
    aged_file(path.with_name(path.name + '.sha256'), f'{checksum}  {path.name}\n'.encode(), days)
    return path


class CacheCleanup(unittest.TestCase):
    def test_preview_then_cleanup_keeps_dependencies_profiles_and_recent_cache(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            old = aged_file(root / 'target/debug/incremental/old/session/data')
            age_tree(old.parents[1])
            recent = aged_file(root / 'target/debug/incremental/recent/session/data', days=0)
            deps = aged_file(root / 'target/debug/deps/library.rlib')
            user = aged_file(root / 'profile/projects/data/tasks/task.json')
            python = aged_file(root / 'scripts/__pycache__/build.cpython-312.pyc')
            age_tree(python.parent)
            preview = cleanup.clean_caches(root)
            self.assertEqual(len(preview['items']), 2)
            self.assertTrue(old.exists())
            cleanup.clean_caches(root, apply=True)
            self.assertFalse(old.exists())
            self.assertFalse(python.exists())
            for path in (recent, deps, user):
                self.assertTrue(path.exists())
            self.assertEqual(cleanup.clean_caches(root, apply=True)['items'], [])

    def test_fresh_descendant_keeps_older_parent_cache(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            file = aged_file(root / 'target/debug/incremental/current/session/data', days=0)
            when = time.time() - 10 * 86400
            os.utime(file.parents[1], (when, when))
            self.assertEqual(cleanup.clean_caches(root, apply=True)['items'], [])
            self.assertTrue(file.exists())

    @unittest.skipIf(os.name == 'nt', 'Unix flock and symlink boundary')
    def test_busy_cargo_profile_is_skipped(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            file = aged_file(root / 'target/debug/incremental/old/data')
            age_tree(file.parent)
            with cleanup.directory_lock(root / 'target/debug/.cargo-lock'):
                result = cleanup.clean_caches(root, apply=True)
            self.assertEqual(result['busy'], [str(root / 'target/debug')])
            self.assertTrue(file.exists())

    @unittest.skipIf(os.name == 'nt', 'Symlink permission depends on Windows runner')
    def test_linked_target_or_profile_never_reaches_external_cache(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root, external = base / 'repo', base / 'external'
            root.mkdir()
            file = aged_file(external / 'debug/incremental/old/data')
            age_tree(file.parent)
            (root / 'target').symlink_to(external, target_is_directory=True)
            self.assertEqual(cleanup.clean_caches(root, apply=True)['items'], [])
            self.assertTrue(file.exists())
            (root / 'target').unlink()
            (root / 'target').mkdir()
            (root / 'target' / TARGET).symlink_to(external, target_is_directory=True)
            self.assertEqual(cleanup.clean_caches(root, apply=True)['items'], [])
            self.assertTrue(file.exists())

    def test_target_triple_incremental_is_pruned(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            file = aged_file(root / 'target' / TARGET / 'debug/incremental/old/data')
            age_tree(file.parent)
            cleanup.clean_caches(root, apply=True)
            self.assertFalse(file.exists())


class ArchiveCleanup(unittest.TestCase):
    def test_verified_windows_package_replaces_only_its_platforms_previous_bundle(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = 'x86_64-pc-windows-msvc'
            for relative in ('scripts/install.py', 'scripts/hook.py', 'README.md',
                             'herdr-plugin.toml', 'LICENSE', 'CHANGELOG.md',
                             'crates/herdr-work-projects/LICENSE',
                             'crates/herdr-work-code/LICENSE', 'crates/herdr-work-engine/LICENSE-process-runtime',
                             f'target/{target}/release/herdr-work.exe'):
                file = root / relative
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b'fixture')
            plan = {'product': 'HerdrWork', 'version': '0.1.0', 'target': target,
                    'sha': 'a' * 40, 'projects_backend': False}
            with patch.object(package, 'ROOT', root), \
                 patch.object(package, 'metadata', side_effect=lambda *a: dict(plan)), \
                 patch.dict(os.environ, {'CARGO_TARGET_DIR': str(root / 'target')}):
                old = package.package(target, root / 'dist')
                other = archive_pair(root / 'dist', '0.1.0', target=LINUX)
                plan['version'] = '0.2.0'
                current = package.package(target, root / 'dist')
            self.assertFalse(old.exists())
            self.assertTrue(other.exists())
            manifest = package.verify(current)
            self.assertFalse(manifest['projects_backend'])
            self.assertNotIn('bin/herdr-work-projects', manifest['files'])

    def test_retains_new_archive_other_platform_and_unowned_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            old = archive_pair(root, '0.1.0', days=10)
            current = archive_pair(root, '0.2.0')
            other = archive_pair(root, '0.1.0', target=LINUX, days=10)
            unrelated = aged_file(root / 'notes.txt')
            unowned = aged_file(root / f'herdr-work-0.0.1-{TARGET}.tar.gz')
            unowned.with_name(unowned.name + '.sha256').write_bytes(b'\xffinvalid checksum')
            cleanup.prune_archives(root, TARGET, protect=current)
            self.assertTrue(old.exists())
            cleanup.prune_archives(root, TARGET, apply=True, protect=current)
            self.assertFalse(old.exists())
            self.assertFalse(old.with_name(old.name + '.sha256').exists())
            for path in (current, other, unrelated, unowned):
                self.assertTrue(path.exists())

    def test_keep_count_and_protected_archive_override_timestamps(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            first = archive_pair(root, '0.1.0', days=3)
            second = archive_pair(root, '0.2.0', days=2)
            third = archive_pair(root, '0.3.0', days=1)
            cleanup.prune_archives(root, TARGET, keep=2, apply=True, protect=first)
            self.assertTrue(first.exists())
            self.assertFalse(second.exists())
            self.assertTrue(third.exists())
            with self.assertRaises(ValueError):
                cleanup.prune_archives(root, TARGET, keep=0)

    def test_failed_publication_restores_previous_archive_and_checksum(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output, stage = root / 'dist', root / 'stage'
            output.mkdir(); stage.mkdir()
            old = archive_pair(output, '0.1.0')
            candidate = archive_pair(stage, '0.1.0')
            old.write_bytes(b'previous archive')
            checksum = old.with_name(old.name + '.sha256')
            checksum.write_bytes(b'previous checksum')
            replace = os.replace
            def fail_checksum(source, dest):
                if Path(source).name == candidate.name + '.sha256':
                    raise OSError('simulated publication failure')
                return replace(source, dest)
            with patch.object(package.os, 'replace', side_effect=fail_checksum):
                with self.assertRaisesRegex(OSError, 'publication failure'):
                    package.publish_archive(candidate, output)
            self.assertEqual(old.read_bytes(), b'previous archive')
            self.assertEqual(checksum.read_bytes(), b'previous checksum')

    def test_failed_candidate_never_prunes_existing_archive(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            old = archive_pair(output, '0.1.0')
            with patch.object(package, 'create_archive', side_effect=ValueError('bad candidate')):
                with self.assertRaisesRegex(ValueError, 'bad candidate'):
                    package.package(TARGET, output)
            self.assertTrue(old.exists())


class UnifiedBuild(unittest.TestCase):
    def test_both_unix_binaries_share_one_cargo_invocation(self):
        with patch.object(build, 'metadata', return_value={'projects_backend': True}), \
             patch.object(build, 'clean_caches', return_value={'items': [], 'busy': []}), \
             patch.object(build.subprocess, 'run') as run:
            build.build(TARGET)
        run.assert_called_once()
        command = run.call_args.args[0]
        self.assertIn('--release', command)
        self.assertIn('--locked', command)
        self.assertIn('herdr-work', command)
        self.assertIn('herdr-work-projects', command)

    def test_windows_build_omits_unix_backend(self):
        with patch.object(build, 'metadata', return_value={'projects_backend': False}), \
             patch.object(build, 'clean_caches', return_value={'items': [], 'busy': []}), \
             patch.object(build.subprocess, 'run') as run:
            build.build('x86_64-pc-windows-msvc')
        self.assertNotIn('herdr-work-projects', run.call_args.args[0])

    def test_target_dir_uses_same_custom_location_for_build_and_package(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with patch.dict(os.environ, {'CARGO_TARGET_DIR': 'custom-target'}):
                self.assertEqual(build.target_dir(root), root / 'custom-target')
            with patch.dict(os.environ, {'CARGO_TARGET_DIR': str(root / 'absolute')}):
                self.assertEqual(build.target_dir(root), root / 'absolute')


if __name__ == '__main__':
    unittest.main()
