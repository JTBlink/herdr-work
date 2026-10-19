"""Run original Projects lifecycle hooks in the isolated HerdrWork data directory."""
import os
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
home = Path(json.loads(subprocess.check_output([str(root / 'bin/herdr-work'), 'profile-path'], text=True, timeout=60))['profile']).resolve()
projects = home / 'projects'
env = dict(os.environ, HERDR_WORK_HOME=str(home), HERDR_PROJECTS_HOME=str(projects),
           HERDR_PROJECTS_DATA_DIR=str(projects / 'data'),
           HERDR_PROJECTS_CONFIG_DIR=str(projects / 'settings'),
           HERDR_PROJECTS_REPORTS_DIR=str(projects / 'reports'),
           HERDR_PLUGIN_ID='jtblink.herdr-work', HERDR_WORK_MANAGED='1')
subprocess.run([str(root / 'bin/herdr-work-projects'), 'hook'], env=env, check=True, timeout=60)
