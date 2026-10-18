"""Run original Projects lifecycle hooks in the isolated Workdesk data directory."""
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
home = Path(os.environ.get('WORKDESK_HOME', str(Path.home() / '.workdesk'))).expanduser().resolve()
projects = home / 'projects'
env = dict(os.environ, HERDR_PROJECTS_HOME=str(projects),
           HERDR_PROJECTS_DATA_DIR=str(projects / 'data'),
           HERDR_PROJECTS_CONFIG_DIR=str(projects / 'settings'),
           HERDR_PROJECTS_REPORTS_DIR=str(projects / 'reports'),
           HERDR_PLUGIN_ID='jtblink.workdesk', WORKDESK_MANAGED='1')
subprocess.run([str(root / 'bin/workdesk-projects'), 'hook'], env=env, check=True, timeout=60)
