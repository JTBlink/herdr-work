"""Draft-first, publish-last release of one verified source SHA."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
from build import ROOT, TARGETS
from package import digest, verify


def changelog_entry(changelog, tag):
    """Select one exact version; never publish Unreleased or older entries."""
    version = tag.removeprefix('v')
    entries = []
    for section in re.split(r'^##[ \t]+', changelog, flags=re.MULTILINE)[1:]:
        heading, _, body = section.partition('\n')
        if re.match(r'v?' + re.escape(version) + r'(?:[ \t]|$)', heading):
            entries.append(body.strip())
    if len(entries) != 1 or not entries[0]:
        raise ValueError(f'Expected one non-empty CHANGELOG entry for {tag}')
    return entries[0]


def release_notes(changelog, tag, sha, repo):
    entry = changelog_entry(changelog, tag)
    return (f'## 本版更新\n\n{entry}\n\n'
            f'[完整更新记录](https://github.com/{repo}/blob/{tag}/CHANGELOG.md)\n\n'
            '<details>\n<summary>平台与校验信息</summary>\n\n'
            'macOS / Linux 包含 Projects 执行后端；Windows 提供文件、Git 与审查功能，'
            '不包含 Projects 执行后端。\n\n'
            '归档包含许可证、`MANIFEST.json` 和用户目录安装器；'
            '`SHA256SUMS` 提供附件校验和。\n\n'
            f'构建源码：`{sha}`\n\n</details>\n')


def publish(assets, tag, sha, repo):
    notes = release_notes((ROOT / 'CHANGELOG.md').read_text(encoding='utf-8'), tag, sha, repo)
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
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'notes.md';p.write_text(notes, encoding='utf-8')
            command=['gh','release','create',tag,'--repo',repo,'--verify-tag','--draft','--title','HerdrWork '+tag,'--notes-file',str(p)]
            if '-' in tag:command.append('--prerelease')
            subprocess.run(command,check=True)
        current={'assets':[]}
    # Refresh a resumed draft as well, including drafts created by older scripts.
    if existing.returncode == 0:
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp) / 'notes.md'
            p.write_text(notes, encoding='utf-8')
            gh('release', 'edit', tag, '--notes-file', str(p))
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
