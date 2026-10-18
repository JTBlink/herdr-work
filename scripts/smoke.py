"""Install a real candidate archive in a temporary user prefix and run its CLI."""
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


def smoke(archive):
    manifest=verify(archive)
    with tempfile.TemporaryDirectory(prefix='workdesk-install-test-') as temp:
        root=Path(temp);extract=root/'archive';extract.mkdir()
        if str(archive).endswith('.zip'):
            with zipfile.ZipFile(archive) as z:z.extractall(extract)
        else:
            with tarfile.open(archive) as t:t.extractall(extract,filter='data')
        stage=next(extract.iterdir());prefix=root/'installed';install(stage,prefix)
        binary=prefix/'bin'/('workdesk.exe' if 'windows' in manifest['target'] else 'workdesk')
        version=subprocess.check_output([str(binary),'--version'],text=True)
        if manifest['version'] not in version:raise ValueError('Installed binary version differs')
        result=json.loads(subprocess.check_output([str(binary),'--home',str(root/'profile'),'doctor'],text=True))
        if result['product']!='Workdesk':raise ValueError('Installed application identity differs')
        if manifest['projects_backend']:
            if not result['projects_backend_installed']:raise ValueError('Installed backend missing')
            tasks=json.loads(subprocess.check_output([str(binary),'--home',str(root/'profile'),'tasks'],text=True))
            if tasks!=[]:raise ValueError('Fresh isolated profile is not empty')
            fixture=root/'repository';fixture.mkdir()
            subprocess.run(['git','init','-q',str(fixture)],check=True)
            env={k:v for k,v in os.environ.items() if not k.startswith('HERDR_')}
            env.update(HERDR_PROJECTS_INLINE='1',HERDR_PROJECTS_NO_KEEPER='1')
            subprocess.check_output([str(binary),'--home',str(root/'profile'),'task','create','--root',str(fixture),'Installation smoke task'],env=env,text=True)
            tasks=json.loads(subprocess.check_output([str(binary),'--home',str(root/'profile'),'tasks'],env=env,text=True))
            if not any(task['title']=='Installation smoke task' for task in tasks):raise ValueError('Original Projects create/list flow failed')
            for action,paused in [('pause',True),('status',True),('resume',False)]:
                state=json.loads(subprocess.check_output([str(binary),'--home',str(root/'profile'),'scheduler',action],env=env,text=True))
                if state['paused']!=paused:raise ValueError('Scheduler state did not persist')
        install(stage,prefix)
        rollback(prefix)
        subprocess.run([str(binary),'--version'],check=True)
        subprocess.run([sys.executable,str(stage/'install.py'),'--prefix',str(prefix),'--uninstall'],check=True)
        if binary.exists():raise ValueError('Uninstall left application binary')
        if not (root/'profile').exists():raise ValueError('Uninstall removed user data')
    print(f'Installed, executed and uninstalled {Path(archive).name}')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--archives',required=True);a=p.parse_args()
    archives=[*Path(a.archives).glob('*.tar.gz'),*Path(a.archives).glob('*.zip')]
    if len(archives)!=1:raise ValueError('Smoke requires exactly one native platform archive')
    smoke(archives[0])
