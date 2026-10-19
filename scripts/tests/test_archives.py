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
    def archive(self, root, *, backend=True, corrupt=False, product='HerdrWork'):
        files={'bin/herdr-work':b'compiled-herdr-work-fixture','install.py':b'# installer','LICENSE':b'MIT'}
        for name in ['herdr-projects','herdr-sidebar','herdr-reviewr']:files[f'licenses/{name}.txt']=b'MIT'
        if backend:files['bin/herdr-work-projects']=b'compiled-backend-fixture'
        manifest={'product':product,'version':'0.1.0','sha':'a'*40,'target':'x86_64-unknown-linux-gnu','projects_backend':True,
                  'files':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}
        files['MANIFEST.json']=json.dumps(manifest).encode()
        if corrupt:files['bin/herdr-work']=b'tampered'
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

    def test_previous_product_identity_is_not_accepted(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(ValueError, 'Unsupported product identity'):
                verify(self.archive(Path(temp), product='Workdesk'))

class InstallerOwnership(unittest.TestCase):
    def test_upgrade_must_not_replace_new_unowned_destination(self):
        from install import install
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();stage=root/'archive';prefix=root/'prefix'
            (stage/'bin').mkdir(parents=True);(prefix/'bin').mkdir(parents=True)
            files={'bin/herdr-work':b'new app','bin/herdr-work-projects':b'new backend'}
            for name,data in files.items():(stage/name).write_bytes(data)
            (stage/'MANIFEST.json').write_text(json.dumps({'product':'HerdrWork','version':'0.1.0','sha':'a'*40,'files':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}))
            owned=prefix/'bin/herdr-work';owned.write_bytes(b'previous app')
            unowned=prefix/'bin/herdr-work-projects';unowned.write_bytes(b'unrelated existing command')
            (prefix/'herdr-work-install.json').write_text(json.dumps({'version':'0.0.1','sha':'b'*40,'files':[str(owned)]}))
            with self.assertRaisesRegex(ValueError,'Unowned executable'):install(stage,prefix)
            self.assertEqual(owned.read_bytes(),b'previous app')
            self.assertEqual(unowned.read_bytes(),b'unrelated existing command')

class InstallerLifecycle(unittest.TestCase):
    def fixture(self, root, suffix=''):
        stage, prefix = root / 'archive', root / 'prefix'
        (stage / 'bin').mkdir(parents=True)
        (prefix / 'bin').mkdir(parents=True)
        name = 'herdr-work' + suffix
        files = {'bin/' + name: b'new app'}
        if not suffix:
            files['bin/herdr-work-projects'] = b'new backend'
        for rel, data in files.items():
            (stage / rel).write_bytes(data)
            (prefix / rel).write_bytes(b'previous ' + data)
        manifest = {'product': 'HerdrWork', 'version': '0.2.0', 'sha': 'a' * 40,
                    'files': {rel: hashlib.sha256(data).hexdigest() for rel, data in files.items()}}
        (stage / 'MANIFEST.json').write_text(json.dumps(manifest))
        receipt = {'version': '0.1.0', 'sha': 'b' * 40,
                   'files': [str(prefix / rel) for rel in files]}
        (prefix / 'herdr-work-install.json').write_text(json.dumps(receipt))
        return stage, prefix, receipt

    def test_same_product_replacement_rollback_and_uninstall_keep_user_data(self):
        import install as installer
        from unittest.mock import patch
        import shutil
        for suffix in ['', '.exe']:
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as temp:
                stage, prefix, previous = self.fixture(Path(temp).resolve(), suffix)
                data = prefix / 'user-data.json'
                data.write_text('keep')
                binary = prefix / 'bin' / ('herdr-work' + suffix)
                with patch('install.replace', side_effect=shutil.copy2):
                    installer.install(stage, prefix)
                    self.assertEqual(binary.read_bytes(), b'new app')
                    installer.rollback(prefix)
                self.assertEqual(binary.read_bytes(), b'previous new app')
                self.assertEqual(installer.read_receipt(prefix), previous)
                installer.uninstall(prefix)
                self.assertFalse(binary.exists())
                self.assertEqual(data.read_text(), 'keep')

    def test_interrupted_replacement_restores_original_installation(self):
        import install as installer
        from unittest.mock import patch
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, previous = self.fixture(Path(temp).resolve())
            calls = 0
            def fail_once(source, dest):
                nonlocal calls
                calls += 1
                if calls == 2:
                    raise OSError('simulated replacement failure')
                shutil.copy2(source, dest)
            with patch('install.replace', side_effect=fail_once):
                with self.assertRaisesRegex(OSError, 'simulated replacement failure'):
                    installer.install(stage, prefix)
            self.assertEqual((prefix / 'bin/herdr-work').read_bytes(), b'previous new app')
            self.assertEqual(installer.read_receipt(prefix), previous)

    def test_receipt_failure_restores_original_installation(self):
        import install as installer
        from unittest.mock import patch
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, previous = self.fixture(Path(temp).resolve())
            original = installer.write_receipt
            calls = 0
            def fail_once(prefix, data):
                nonlocal calls
                calls += 1
                if calls == 1:
                    raise OSError('simulated receipt failure')
                original(prefix, data)
            with patch('install.replace', side_effect=shutil.copy2), patch('install.write_receipt', side_effect=fail_once):
                with self.assertRaisesRegex(OSError, 'simulated receipt failure'):
                    installer.install(stage, prefix)
            self.assertEqual((prefix / 'bin/herdr-work').read_bytes(), b'previous new app')
            self.assertEqual(installer.read_receipt(prefix), previous)

    def test_old_receipt_is_not_adopted_and_existing_files_are_untouched(self):
        import install as installer
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, previous = self.fixture(Path(temp).resolve())
            (prefix / 'herdr-work-install.json').rename(prefix / 'workdesk-install.json')
            with self.assertRaisesRegex(ValueError, 'Unowned executable'):
                installer.install(stage, prefix)
            self.assertEqual((prefix / 'bin/herdr-work').read_bytes(), b'previous new app')
            self.assertTrue((prefix / 'workdesk-install.json').is_file())
            self.assertFalse((prefix / 'herdr-work-install.json').exists())

    def test_current_receipt_rejects_obsolete_command_names(self):
        import install as installer
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, previous = self.fixture(Path(temp).resolve())
            obsolete = prefix / 'bin/workdesk-projects'
            obsolete.write_bytes(b'unmanaged old executable')
            previous['files'].append(str(obsolete))
            (prefix / 'herdr-work-install.json').write_text(json.dumps(previous))
            with self.assertRaisesRegex(ValueError, 'Invalid install receipt'):
                installer.install(stage, prefix)
            self.assertEqual(obsolete.read_bytes(), b'unmanaged old executable')
            self.assertEqual((prefix / 'bin/herdr-work').read_bytes(), b'previous new app')

if __name__ == '__main__':
    unittest.main()
