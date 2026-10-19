"""Reviewr-inspired draft-first, publish-last release of one verified source SHA."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
from build import ROOT, TARGETS
from package import digest, verify


def publish(assets, tag, sha, repo):
    assets = Path(assets)
    archives = sorted([*assets.glob('*.tar.gz'), *assets.glob('*.zip')])
    manifests = [verify(p) for p in archives]
    if len(manifests) != len(TARGETS) or {m['target'] for m in manifests} != set(TARGETS):
        raise ValueError('Every declared platform is required before publishing')
    if any(m.get('product') != 'HerdrWork' for m in manifests):
        raise ValueError('Release product identity differs')
    if any(m['sha'] != sha or f'v{m["version"]}' != tag for m in manifests):
        raise ValueError('Mixed commits or versions in release assets')
    def gh(*args):
        return subprocess.run(['gh', *args, '--repo', repo], check=True, text=True, capture_output=True).stdout
    existing = subprocess.run(['gh','release','view',tag,'--repo',repo,'--json','isDraft,assets'],text=True,capture_output=True)
    if existing.returncode == 0:
        current = json.loads(existing.stdout)
        if not current['isDraft']: raise ValueError('Release is already public; refusing to overwrite')
    else:
        # A failed network request must not be treated as proof the release is absent.
        subprocess.run(['gh','api',f'repos/{repo}/git/ref/tags/{tag}'],check=True,capture_output=True)
        notes = ('HerdrWork '+tag+'\n\nSource: '+sha+'\n\n'
                 'macOS/Linux include the original Projects execution backend. Windows includes the native files/Git/review HerdrWork; Projects task execution remains Unix-only.\n'
                 'Each archive includes original licenses, MANIFEST.json and a user-prefix installer. SHA256SUMS covers all assets.\n\n'
                 + (ROOT/'CHANGELOG.md').read_text())
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'notes.md';p.write_text(notes)
            command=['gh','release','create',tag,'--repo',repo,'--verify-tag','--draft','--title','HerdrWork '+tag,'--notes-file',str(p)]
            if '-' in tag:command.append('--prerelease')
            subprocess.run(command,check=True)
        current={'assets':[]}
    files=[*archives,*[p.with_name(p.name+'.sha256') for p in archives]]
    sums=assets/'SHA256SUMS';sums.write_text(''.join(f'{digest(p)}  {p.name}\n' for p in files))
    files.append(sums)
    known={a['name'] for a in current['assets']}
    for file in files:
        if file.name in known:
            with tempfile.TemporaryDirectory() as temp:
                gh('release','download',tag,'--pattern',file.name,'--dir',temp)
                if digest(Path(temp)/file.name) != digest(file): raise ValueError('Existing draft asset differs; refusing to overwrite')
        else: gh('release','upload',tag,str(file))
    gh('release','edit',tag,'--draft=false')

if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--assets',required=True);a=p.parse_args()
    publish(a.assets,os.environ['RELEASE_TAG'],os.environ['RELEASE_SHA'],os.environ['GH_REPO'])
