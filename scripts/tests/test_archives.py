"""Artifact verification against independent fixture archives and deliberate corruptions."""
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from package import verify

class Archives(unittest.TestCase):
    def archive(self, root, *, backend=True, corrupt=False):
        files={'bin/workdesk':b'compiled-workdesk-fixture','install.py':b'# installer','LICENSE':b'MIT'}
        for name in ['herdr-projects','herdr-sidebar','herdr-reviewr']:files[f'licenses/{name}.txt']=b'MIT'
        if backend:files['bin/workdesk-projects']=b'compiled-backend-fixture'
        manifest={'product':'workdesk','version':'0.1.0','sha':'a'*40,'target':'x86_64-unknown-linux-gnu','projects_backend':True,
                  'files':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}
        files['MANIFEST.json']=json.dumps(manifest).encode()
        if corrupt:files['bin/workdesk']=b'tampered'
        path=root/'fixture.tar.gz'
        with tarfile.open(path,'w:gz') as tar:
            for name,data in files.items():
                info=tarfile.TarInfo('fixture/'+name);info.size=len(data);tar.addfile(info,io.BytesIO(data))
        sha=hashlib.sha256(path.read_bytes()).hexdigest();path.with_name(path.name+'.sha256').write_text(f'{sha}  {path.name}\n')
        return path
    def test_valid_archive_has_original_licenses_and_backend(self):
        with tempfile.TemporaryDirectory() as temp:
            info=verify(self.archive(Path(temp)))
            self.assertEqual(info['version'],'0.1.0')
    def test_missing_backend_cannot_be_packaged_as_unix_distribution(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(ValueError,'Required executable'):verify(self.archive(Path(temp),backend=False))
    def test_inner_binary_tampering_is_detected_even_with_new_outer_checksum(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(ValueError,'Manifest checksum'):verify(self.archive(Path(temp),corrupt=True))
    def test_outer_checksum_is_mandatory(self):
        with tempfile.TemporaryDirectory() as temp:
            path=self.archive(Path(temp));path.with_name(path.name+'.sha256').write_text('0'*64+'  fixture.tar.gz\n')
            with self.assertRaisesRegex(ValueError,'Archive checksum'):verify(path)
if __name__=='__main__':unittest.main()

class InstallerOwnership(unittest.TestCase):
    def test_upgrade_must_not_replace_new_unowned_destination(self):
        from install import install
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();stage=root/'archive';prefix=root/'prefix'
            (stage/'bin').mkdir(parents=True);(prefix/'bin').mkdir(parents=True)
            files={'bin/workdesk':b'new app','bin/workdesk-projects':b'new backend'}
            for name,data in files.items():(stage/name).write_bytes(data)
            (stage/'MANIFEST.json').write_text(json.dumps({'version':'0.1.0','sha':'a'*40,'files':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}))
            owned=prefix/'bin/workdesk';owned.write_bytes(b'previous app')
            unowned=prefix/'bin/workdesk-projects';unowned.write_bytes(b'unrelated existing command')
            (prefix/'workdesk-install.json').write_text(json.dumps({'version':'0.0.1','sha':'b'*40,'files':[str(owned)]}))
            with self.assertRaisesRegex(ValueError,'Unowned executable'):install(stage,prefix)
            self.assertEqual(owned.read_bytes(),b'previous app')
            self.assertEqual(unowned.read_bytes(),b'unrelated existing command')
