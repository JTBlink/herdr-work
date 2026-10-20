"""Default shortcut installation preserves host settings and user decisions."""
import os
from pathlib import Path
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from configure_keys import COMMAND, config_path, configure


class DefaultShortcut(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.config = self.root / 'herdr/config.toml'
        self.config.parent.mkdir()
        self.profile = self.root / 'product'

    def write(self, text):
        self.config.write_text(text)
        return self.config.read_bytes()

    def test_install_preserves_original_text_and_other_plugin_bindings(self):
        original = self.write('# my theme\n[theme]\nname = "terminal"\n'
                              '[keys]\nprefix = "ctrl+a"\n'
                              '[[keys.command]]\nkey = "cmd+p"\ntype = "shell"\n'
                              'command = "herdr plugin action invoke quick-open --plugin herdr-sidebar"\n')
        self.assertEqual(configure(self.config, self.profile), ('added', 'ctrl+a'))
        self.assertTrue(self.config.read_bytes().startswith(original))
        entries = tomllib.loads(self.config.read_text())['keys']['command']
        self.assertEqual(entries[-1]['command'], COMMAND)
        self.assertEqual(entries[-1]['key'], 'prefix+f')
        self.assertIn('herdr-sidebar', entries[0]['command'])
        backups = list(self.config.parent.glob('*.herdr-work-backup-*'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), original)

    def test_repeat_install_and_upgrade_preserve_edits_and_removal(self):
        configure(self.config, self.profile)
        original = self.config.read_bytes()
        self.assertEqual(configure(self.config, self.profile)[0], 'preserved')
        self.assertEqual(self.config.read_bytes(), original)
        for edited in [original.replace(b'prefix+f', b'prefix+o'), b'# user removed binding\n']:
            self.config.write_bytes(edited)
            self.assertEqual(configure(self.config, self.profile)[0], 'preserved')
            self.assertEqual(self.config.read_bytes(), edited)

    def test_conflicting_commands_and_builtin_bindings_are_untouched(self):
        for text in [
            '[keys]\nsettings = "prefix+f"\n',
            '[keys]\n[[keys.command]]\nkey = "prefix+f"\ntype = "shell"\ncommand = "other"\n',
            '[keys]\n[[keys.command]]\nkey = " PREFIX + F "\ntype = "shell"\ncommand = "other"\n',
        ]:
            with self.subTest(text=text):
                original = self.write(text)
                self.assertEqual(configure(self.config, self.profile)[0], 'conflict')
                self.assertEqual(self.config.read_bytes(), original)

    def test_existing_custom_product_binding_is_not_duplicated(self):
        original = self.write('[[keys.command]]\nkey = "alt+e"\ntype = "shell"\n'
                              f'command = "{COMMAND}"\n')
        self.assertEqual(configure(self.config, self.profile)[0], 'preserved')
        self.assertEqual(self.config.read_bytes(), original)

    def test_missing_config_is_created_with_a_valid_binding(self):
        self.assertEqual(configure(self.config, self.profile), ('added', 'ctrl+b'))
        entries = tomllib.loads(self.config.read_text())['keys']['command']
        self.assertEqual(entries, [{'key': 'prefix+f', 'type': 'shell', 'command': COMMAND}])

    def test_invalid_or_sealed_toml_is_not_overwritten(self):
        for text in ['[keys', 'keys = { prefix = "ctrl+b" }\n']:
            with self.subTest(text=text):
                original = self.write(text)
                with self.assertRaises(ValueError):
                    configure(self.config, self.profile)
                self.assertEqual(self.config.read_bytes(), original)

    def test_explicit_host_config_path_wins(self):
        custom = self.root / 'custom.toml'
        with patch.dict(os.environ, {'HERDR_CONFIG_PATH': str(custom),
                                     'XDG_CONFIG_HOME': str(self.root / 'xdg')}):
            self.assertEqual(config_path(), custom.resolve())
