"""The installer delegates registration to Herdr without recursively running its build hook."""
from pathlib import Path
import subprocess
import sys
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import plugin


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

    def test_manifest_calls_build_hook_instead_of_install_entrypoint(self):
        manifest = tomllib.loads((plugin.ROOT / 'herdr-plugin.toml').read_text())
        self.assertEqual(manifest['build'], [{'command': ['python3', 'scripts/plugin_build.py']}])
