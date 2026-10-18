"""Install a native archive and exercise the actual CLI and original Projects launcher."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from package import verify
from install import install, rollback


def output(command, **kwargs):
    return subprocess.check_output([str(item) for item in command], text=True, timeout=60, **kwargs)


def projects_smoke(binary, backend, profile, root):
    command = [binary, '--home', profile]
    env = {k: v for k, v in os.environ.items() if not k.startswith(('HERDR_', 'WORKDESK_'))}
    env.update(HERDR_PROJECTS_INLINE='1', HERDR_PROJECTS_NO_KEEPER='1')
    if json.loads(output(command + ['tasks'], env=env)) != []:
        raise ValueError('Fresh isolated profile is not empty')
    fixture = root / 'repository'
    fixture.mkdir()
    subprocess.run(['git', 'init', '-q', str(fixture)], check=True)
    created = json.loads(output(command + ['task', 'create', '--root', fixture, 'Installation smoke task'], env=env))
    projects = (profile / 'projects').resolve()
    task_file = projects / 'data' / 'tasks' / created['id'] / 'task.json'
    task = json.loads(task_file.read_text())
    if Path(task['root']).resolve() != fixture.resolve():
        raise ValueError('Task creation ignored the explicitly selected repository')
    tasks = json.loads(output(command + ['tasks'], env=env))
    if not any(row['title'] == 'Installation smoke task' for row in tasks):
        raise ValueError('Original Projects create/list flow failed')

    managed = dict(env, WORKDESK_MANAGED='1', HERDR_PLUGIN_ID='jtblink.workdesk',
                   HERDR_PROJECTS_HOME=str(projects), HERDR_PROJECTS_DATA_DIR=str(projects / 'data'),
                   HERDR_PROJECTS_CONFIG_DIR=str(projects / 'settings'),
                   HERDR_PROJECTS_REPORTS_DIR=str(projects / 'reports'))
    launcher = json.loads(output([backend, 'workdesk-launcher'], env=managed))['launcher']
    socket = str(root / 'private-missing.sock')
    task.update(link={'pane': 'fixture-pane', 'socket': socket, 'terminal': 'fixture-terminal'},
                agent='codex', stage='working')
    task_file.write_text(json.dumps(task))
    # A fresh Agent process must retain isolation while shedding any inherited plugin UI identity.
    callback = dict(env, HERDR_SOCKET_PATH=socket, HERDR_PANE_ID='fixture-pane',
                    HERDR_PLUGIN_ID='stale.plugin.ui',
                    HERDR_PLUGIN_CONTEXT_JSON=json.dumps({'focused_pane_cwd': str(root)}))
    observed = json.loads(output([launcher, 'workdesk-environment'], env=callback, cwd=fixture))
    expected = {'home': str(projects), 'data': str(projects / 'data'),
                'config': str(projects / 'settings'), 'reports': str(projects / 'reports'),
                'managed': True, 'plugin': '', 'context': str(fixture.resolve()),
                'caller': {'task': created['id'], 'role': 'agent'}}
    if observed != expected:
        raise ValueError('Original Agent callback lost isolation or caller/context identity')
    output([launcher, 'report', '--state', 'blocked', '--msg', 'Installation callback fixture', '--json'],
           env=callback, cwd=fixture)
    reported = json.loads(task_file.read_text())
    if reported['stage'] != 'asks' or not reported.get('selfBlocked'):
        raise ValueError('Original no-id report lost Agent blocked-turn semantics')
    for action, paused in [('pause', True), ('status', True), ('resume', False)]:
        state = json.loads(output(command + ['scheduler', action], env=env))
        if state['paused'] != paused:
            raise ValueError('Scheduler state did not persist')


def smoke(archive):
    manifest = verify(archive)
    with tempfile.TemporaryDirectory(prefix='workdesk-install-test-') as temp:
        root = Path(temp)
        extract = root / 'archive'
        extract.mkdir()
        if str(archive).endswith('.zip'):
            with zipfile.ZipFile(archive) as bundle:
                bundle.extractall(extract)
        else:
            with tarfile.open(archive) as bundle:
                bundle.extractall(extract, filter='data')
        stage = next(extract.iterdir())
        prefix = root / 'installed'
        install(stage, prefix)
        binary = prefix / 'bin' / ('workdesk.exe' if 'windows' in manifest['target'] else 'workdesk')
        if manifest['version'] not in output([binary, '--version']):
            raise ValueError('Installed binary version differs')
        profile = root / 'profile'
        result = json.loads(output([binary, '--home', profile, 'doctor']))
        if result['product'] != 'Workdesk':
            raise ValueError('Installed application identity differs')
        if manifest['projects_backend']:
            if not result['projects_backend_installed']:
                raise ValueError('Installed backend missing')
            projects_smoke(binary, prefix / 'bin' / 'workdesk-projects', profile, root)
        install(stage, prefix)
        rollback(prefix)
        output([binary, '--version'])
        subprocess.run([sys.executable, str(stage / 'install.py'), '--prefix', str(prefix), '--uninstall'], check=True)
        if binary.exists() or not profile.exists():
            raise ValueError('Uninstall left the binary or removed user data')
    print(f'Installed, executed and uninstalled {Path(archive).name}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archives', required=True)
    args = parser.parse_args()
    archives = [*Path(args.archives).glob('*.tar.gz'), *Path(args.archives).glob('*.zip')]
    if len(archives) != 1:
        raise ValueError('Smoke requires exactly one native platform archive')
    smoke(archives[0])
