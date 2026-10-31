"""The installer delegates registration to Herdr without recursively running its build hook."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import plugin
import plugin_build
import package


class PluginInstall(unittest.TestCase):
    def test_release_uses_official_install_with_current_version(self):
        with patch('plugin.subprocess.run') as run:
            plugin.install_plugin()
        version = (plugin.ROOT / 'VERSION').read_text().strip()
        run.assert_called_once_with(['herdr', 'plugin', 'install', 'JTBlink/herdr-work',
                                     '--ref', f'v{version}'], check=True)

    def test_local_build_then_official_link(self):
        with patch('plugin.subprocess.run') as run:
            plugin.install_plugin(True)
        self.assertEqual(run.call_args_list[0].args[0],
                         [sys.executable, str(plugin.ROOT / 'scripts/plugin_build.py'), '--from-source'])
        self.assertEqual(run.call_args_list[1].args[0], ['herdr', 'plugin', 'link', str(plugin.ROOT)])

    def test_failed_build_does_not_register_the_plugin(self):
        with patch('plugin.subprocess.run', side_effect=subprocess.CalledProcessError(1, 'build')) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                plugin.install_plugin(True)
        self.assertEqual(run.call_count, 1)

    def test_build_only_never_registers_plugin(self):
        with patch('plugin.subprocess.run') as run:
            plugin.install_plugin(build_only=True)
        run.assert_called_once_with(
            [sys.executable, str(plugin.ROOT / 'scripts/plugin_build.py'), '--from-source', '--build-only'],
            cwd=plugin.ROOT, check=True)

    def test_manifest_calls_build_hook_instead_of_install_entrypoint(self):
        manifest = tomllib.loads((plugin.ROOT / 'herdr-plugin.toml').read_text())
        self.assertEqual(manifest['build'], [{'command': ['python3', 'scripts/plugin_build.py']}])


class SourcePackaging(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        self.target = 'x86_64-unknown-linux-gnu'
        self.plan = {'product': 'HerdrWork', 'version': '0.1.0', 'sha': 'a' * 40,
                     'target': self.target, 'projects_backend': True}
        for relative in ('scripts/install.py', 'scripts/hook.py', 'README.md',
                         'herdr-plugin.toml', 'LICENSE', 'CHANGELOG.md',
                         'crates/herdr-work-projects/LICENSE', 'crates/herdr-work-code/LICENSE',
                         'crates/herdr-work-engine/LICENSE-process-runtime'):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture')
        for patcher in (
                patch.object(plugin_build, 'ROOT', self.root),
                patch.object(package, 'ROOT', self.root),
                patch.object(package, 'metadata', side_effect=lambda *a: dict(self.plan)),
                patch.dict(os.environ, {'CARGO_TARGET_DIR': str(self.root / 'target')}),
                patch('plugin_build.platform.system', return_value='Linux'),
                patch('plugin_build.platform.machine', return_value='x86_64')):
            patcher.start()
            self.addCleanup(patcher.stop)
        self.compiler = self.enterContext(patch('plugin_build.subprocess.run', side_effect=self.compile))
        self.keys = self.enterContext(patch('configure_keys.setup'))

    def compile(self, command, **kwargs):
        self.assertIn('build.py', command[1])
        for name in ('herdr-work', 'herdr-work-projects'):
            binary = self.root / 'target' / self.target / 'release' / name
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_text(self.plan['version'])
        return subprocess.CompletedProcess(command, 0)

    def test_build_only_retains_verified_archive_without_install_or_config_changes(self):
        archive = plugin_build.prepare(True, build_only=True)
        self.assertEqual(archive.parent, self.root / 'dist')
        self.assertEqual(package.verify(archive)['version'], '0.1.0')
        self.assertTrue(archive.with_name(archive.name + '.sha256').is_file())
        self.assertFalse((self.root / 'bin').exists())
        self.assertFalse((self.root / 'herdr-work-install.json').exists())
        self.keys.assert_not_called()
        self.compiler.assert_called_once()

    def test_default_source_install_retains_only_latest_archive_and_installs_its_binaries(self):
        old = plugin_build.prepare(True, build_only=True)
        self.plan['version'] = '0.2.0'
        plugin_build.prepare(True)
        archives = list((self.root / 'dist').glob('*.tar.gz'))
        self.assertEqual(len(archives), 1)
        self.assertEqual(package.verify(archives[0])['version'], '0.2.0')
        self.assertFalse(old.exists())
        self.assertFalse(old.with_name(old.name + '.sha256').exists())
        for name in ('herdr-work', 'herdr-work-projects'):
            self.assertEqual((self.root / 'bin' / name).read_text(), '0.2.0')
        self.assertTrue((self.root / 'herdr-work-install.json').is_file())
        self.keys.assert_called_once()

    def test_failed_build_keeps_previous_archive_and_skips_installation(self):
        previous = plugin_build.prepare(True, build_only=True)
        checksum = package.digest(previous)
        self.compiler.side_effect = subprocess.CalledProcessError(1, 'cargo')
        with self.assertRaises(subprocess.CalledProcessError):
            plugin_build.prepare(True)
        self.assertEqual(package.digest(previous), checksum)
        package.verify(previous)
        self.assertFalse((self.root / 'bin').exists())
        self.keys.assert_not_called()

    def test_failed_package_keeps_previous_archive_and_skips_installation(self):
        previous = plugin_build.prepare(True, build_only=True)
        with patch.object(package, 'verify', side_effect=ValueError('invalid candidate')):
            with self.assertRaisesRegex(ValueError, 'invalid candidate'):
                plugin_build.prepare(True)
        package.verify(previous)
        self.assertFalse((self.root / 'bin').exists())
        self.keys.assert_not_called()

    def test_unsupported_host_fails_before_build_or_install(self):
        with patch('plugin_build.platform.system', return_value='Windows'):
            with self.assertRaisesRegex(ValueError, 'support'):
                plugin_build.prepare(True)
        self.compiler.assert_not_called()
        self.keys.assert_not_called()


@unittest.skipIf(os.name == 'nt', 'POSIX shell entry point')
class ShellEntry(unittest.TestCase):
    def test_external_working_directory_and_modes_forward_to_the_repository(self):
        script = plugin.ROOT / 'herdr-work-plugins.sh'
        with tempfile.TemporaryDirectory(prefix='herdr-work shell ') as temp:
            root = Path(temp)
            log = root / 'calls.txt'
            for tool in ('python3', 'cargo', 'herdr'):
                stub = root / tool
                stub.write_text('#!/bin/sh\nprintf "%s|%s\\n" "$PWD" "$*" >> "$PLUGIN_TEST_LOG"\n')
                stub.chmod(0o755)
            env = dict(os.environ, PATH=str(root), PLUGIN_TEST_LOG=str(log))
            for option, mode in (([], '--from-source'), (['install'], '--from-source'),
                                 (['--from-source'], '--from-source'), (['--build-only'], '--build-only'),
                                 (['--release'], '--release')):
                with self.subTest(option=option):
                    log.unlink(missing_ok=True)
                    result = subprocess.run(['/bin/sh', str(script), *option], cwd=root,
                                            env=env, text=True, capture_output=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIn(f'{plugin.ROOT}|scripts/plugin.py {mode}', log.read_text())
            (root / 'herdr').unlink()
            result = subprocess.run(['/bin/sh', str(script), '--build-only'], cwd=root,
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            result = subprocess.run(['/bin/sh', str(script)], cwd=root,
                                    env=env, text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('herdr', result.stderr)
            (root / 'cargo').unlink()
            result = subprocess.run(['/bin/sh', str(script), 'clean'], cwd=root,
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f'{plugin.ROOT}|scripts/cleanup.py --apply --max-age-days 0', log.read_text())
            (root / 'herdr').write_text('#!/bin/sh\nexit 0\n')
            (root / 'herdr').chmod(0o755)
            result = subprocess.run(['/bin/sh', str(script), '--release'], cwd=root,
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_help_and_invalid_options_do_not_require_tools(self):
        script = plugin.ROOT / 'herdr-work-plugins.sh'
        with tempfile.TemporaryDirectory() as temp:
            (Path(temp) / 'cat').symlink_to('/bin/cat')
            env = dict(os.environ, PATH=temp)
            for options, code in ((['--help'], 0), (['--unknown'], 2),
                                  (['--release', '--build-only'], 2)):
                with self.subTest(options=options):
                    result = subprocess.run(['/bin/sh', str(script), *options], env=env,
                                            capture_output=True, text=True)
                    self.assertEqual(result.returncode, code)

    def test_pipeline_failure_is_propagated_without_success_message(self):
        script = plugin.ROOT / 'herdr-work-plugins.sh'
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for tool in ('python3', 'cargo'):
                stub = root / tool
                stub.write_text('#!/bin/sh\ncase "$1" in scripts/plugin.py) exit 7;; esac\n')
                stub.chmod(0o755)
            result = subprocess.run(['/bin/sh', str(script), '--build-only'],
                                    env=dict(os.environ, PATH=temp), text=True, capture_output=True)
            self.assertEqual(result.returncode, 7)
            self.assertNotIn('编译打包完成', result.stdout)
