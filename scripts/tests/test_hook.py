"""Hook profile resolution shares the Rust migration/configuration rules."""
import json
import os
from pathlib import Path
import runpy
import tomllib
import unittest
from unittest.mock import patch

class HookProfile(unittest.TestCase):
    def test_manifest_activities_allocate_the_products_own_tty_entrypoint(self):
        root = Path(__file__).resolve().parents[2]
        manifest = tomllib.loads((root / 'herdr-plugin.toml').read_text())
        self.assertEqual(manifest['id'], 'jtblink.herdr-work')
        self.assertEqual(manifest['panes'][0]['command'], ['bin/herdr-work', 'ui', '--sidebar'])
        self.assertEqual(manifest['panes'][0]['placement'], 'split')
        actions = {action['id']: action['command'] for action in manifest['actions']}
        self.assertEqual(set(actions), {'show-explorer', 'show-search', 'show-git', 'quick-open', 'toggle'})
        for name, command in actions.items():
            self.assertEqual(command, ['bin/herdr-work', 'activate', name])

    def test_hook_uses_canonical_cli_profile_and_backend_without_taking_ui_lock(self):
        profile=Path('/tmp/herdr-work-hook-fixture').resolve()
        with patch.dict(os.environ,{},clear=True), patch('subprocess.check_output',return_value=json.dumps({'profile':str(profile)})) as resolve, patch('subprocess.run') as backend:
            runpy.run_path(str(Path(__file__).resolve().parents[1]/'hook.py'))
        self.assertEqual(resolve.call_args.args[0][-1],'profile-path')
        self.assertEqual(Path(resolve.call_args.args[0][0]).name,'herdr-work')
        self.assertEqual(Path(backend.call_args.args[0][0]).name,'herdr-work-projects')
        env=backend.call_args.kwargs['env']
        self.assertEqual(env['HERDR_PROJECTS_HOME'],str(profile/'projects'))
        self.assertEqual(env['HERDR_WORK_MANAGED'],'1')
        self.assertEqual(env['HERDR_PLUGIN_ID'],'jtblink.herdr-work')
