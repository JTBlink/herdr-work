"""Plugin hooks only activate the owned workbench."""
from pathlib import Path
import tomllib
import unittest

class HookProfile(unittest.TestCase):
    def test_closing_or_focusing_panes_does_not_auto_reopen_the_workbench(self):
        root = Path(__file__).resolve().parents[2]
        manifest = tomllib.loads((root / 'herdr-plugin.toml').read_text())
        for event in ['pane.closed', 'pane.focused', 'tab.focused', 'workspace.focused']:
            dispatched = [hook['command'] for hook in manifest['events'] if hook['on'] == event]
            self.assertNotIn(['bin/herdr-work', 'sidebar-ensure'], dispatched, event)
        for event in ['tab.created', 'workspace.created']:
            dispatched = [hook['command'] for hook in manifest['events'] if hook['on'] == event]
            self.assertIn(['bin/herdr-work', 'sidebar-ensure'], dispatched, event)

    def test_manifest_activities_allocate_the_products_own_tty_entrypoint(self):
        root = Path(__file__).resolve().parents[2]
        manifest = tomllib.loads((root / 'herdr-plugin.toml').read_text())
        self.assertEqual(manifest['id'], 'jtblink.herdr-work')
        self.assertEqual(manifest['panes'][0]['command'], ['bin/herdr-work', 'ui', '--sidebar'])
        self.assertEqual(manifest['panes'][0]['placement'], 'split')
        self.assertEqual(len(manifest['panes']), 1)
        self.assertEqual({event['on'] for event in manifest['events']},
                         {'tab.created', 'workspace.created'})
        actions = {action['id']: action['command'] for action in manifest['actions']}
        self.assertEqual(set(actions), {'show-explorer', 'show-search', 'show-git', 'quick-open', 'toggle'})
        for name, command in actions.items():
            self.assertEqual(command, ['bin/herdr-work', 'activate', name])
