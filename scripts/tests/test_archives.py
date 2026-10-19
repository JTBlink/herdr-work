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
        files={'bin/herdr-work':b'compiled-herdr-work-fixture','install.py':b'# installer','LICENSE':b'MIT'}
        if backend:files['bin/herdr-work-projects']=b'compiled-backend-fixture'
        manifest={'product':'HerdrWork','version':'0.1.0','sha':'a'*40,'target':'x86_64-unknown-linux-gnu','projects_backend':True,
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

class InstallerOwnership(unittest.TestCase):
    def test_upgrade_must_not_replace_new_unowned_destination(self):
        from install import install
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();stage=root/'archive';prefix=root/'prefix'
            (stage/'bin').mkdir(parents=True);(prefix/'bin').mkdir(parents=True)
            files={'bin/herdr-work':b'new app','bin/herdr-work-projects':b'new backend'}
            for name,data in files.items():(stage/name).write_bytes(data)
            (stage/'MANIFEST.json').write_text(json.dumps({'version':'0.1.0','sha':'a'*40,'files':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}))
            owned=prefix/'bin/herdr-work';owned.write_bytes(b'previous app')
            unowned=prefix/'bin/herdr-work-projects';unowned.write_bytes(b'unrelated existing command')
            (prefix/'herdr-work-install.json').write_text(json.dumps({'version':'0.0.1','sha':'b'*40,'files':[str(owned)]}))
            with self.assertRaisesRegex(ValueError,'Unowned executable'):install(stage,prefix)
            self.assertEqual(owned.read_bytes(),b'previous app')
            self.assertEqual(unowned.read_bytes(),b'unrelated existing command')

class InstallerRename(unittest.TestCase):
    def fixture(self, root, old_name='workdesk', new_name='herdr-work'):
        stage, prefix = root / 'archive', root / 'prefix'
        (stage / 'bin').mkdir(parents=True)
        (prefix / 'bin').mkdir(parents=True)
        files = {f'bin/{new_name}': b'new app', 'bin/herdr-work-projects': b'new backend'}
        for name, data in files.items():
            (stage / name).write_bytes(data)
        (stage / 'MANIFEST.json').write_text(json.dumps({
            'version': '0.1.2', 'sha': 'a' * 40,
            'files': {name: hashlib.sha256(data).hexdigest() for name, data in files.items()},
        }))
        old = prefix / 'bin' / old_name
        old.write_bytes(b'previous app')
        backend = prefix / 'bin/herdr-work-projects'
        backend.write_bytes(b'previous backend')
        receipt = {'version': '0.1.1', 'sha': 'b' * 40, 'files': [str(old), str(backend)]}
        (prefix / 'herdr-work-install.json').write_text(json.dumps(receipt))
        return stage, prefix, old, prefix / 'bin' / new_name, backend, receipt

    def test_rename_upgrade_and_rollback_restore_exact_owned_files(self):
        from install import install, rollback, uninstall, read_receipt
        from unittest.mock import patch
        import shutil
        for old_name, new_name in [('workdesk', 'herdr-work'), ('workdesk.exe', 'herdr-work.exe')]:
            with self.subTest(new_name=new_name), tempfile.TemporaryDirectory() as temp:
                stage, prefix, old, new, backend, receipt = self.fixture(Path(temp).resolve(), old_name, new_name)
                with patch('install.replace', side_effect=shutil.copy2):
                    install(stage, prefix)
                    self.assertFalse(old.exists())
                    self.assertEqual(new.read_bytes(), b'new app')
                    self.assertEqual(backend.read_bytes(), b'new backend')
                    rollback(prefix)
                self.assertFalse(new.exists())
                self.assertEqual(old.read_bytes(), b'previous app')
                self.assertEqual(backend.read_bytes(), b'previous backend')
                self.assertEqual(read_receipt(prefix), receipt)
                uninstall(prefix)
                self.assertFalse(old.exists())
                self.assertFalse(backend.exists())

    def test_rename_refuses_unowned_new_command(self):
        from install import install, read_receipt
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, old, new, backend, receipt = self.fixture(Path(temp).resolve())
            new.write_bytes(b'unrelated command')
            with self.assertRaisesRegex(ValueError, 'Unowned executable'):
                install(stage, prefix)
            self.assertEqual(read_receipt(prefix), receipt)
            self.assertEqual(old.read_bytes(), b'previous app')
            self.assertEqual(new.read_bytes(), b'unrelated command')

    def test_failed_upgrade_removes_partially_installed_new_name(self):
        from install import install, read_receipt
        from unittest.mock import patch
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, old, new, backend, receipt = self.fixture(Path(temp).resolve())
            calls = 0
            def fail_once(source, dest):
                nonlocal calls
                calls += 1
                if calls == 2:
                    raise OSError('simulated replacement failure')
                shutil.copy2(source, dest)
            with patch('install.replace', side_effect=fail_once):
                with self.assertRaisesRegex(OSError, 'simulated replacement failure'):
                    install(stage, prefix)
            self.assertFalse(new.exists())
            self.assertEqual(old.read_bytes(), b'previous app')
            self.assertEqual(backend.read_bytes(), b'previous backend')
            self.assertEqual(read_receipt(prefix), receipt)

    def test_rollback_refuses_recreated_unowned_legacy_command(self):
        from install import install, rollback, read_receipt
        from unittest.mock import patch
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, old, new, backend, receipt = self.fixture(Path(temp).resolve())
            with patch('install.replace', side_effect=shutil.copy2):
                install(stage, prefix)
                old.write_bytes(b'unrelated recreated command')
                with self.assertRaisesRegex(ValueError, 'Unowned executable'):
                    rollback(prefix)
            self.assertEqual(old.read_bytes(), b'unrelated recreated command')
            self.assertEqual(new.read_bytes(), b'new app')
            self.assertEqual(read_receipt(prefix)['version'], '0.1.2')

    def test_receipt_failure_restores_removed_legacy_command(self):
        import install as installer
        from unittest.mock import patch
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            stage, prefix, old, new, backend, receipt = self.fixture(Path(temp).resolve())
            original = installer.write_receipt
            calls = 0
            def fail_once(prefix, data, *args):
                nonlocal calls
                calls += 1
                if calls == 1:
                    raise OSError('simulated receipt failure')
                original(prefix, data, *args)
            with patch('install.replace', side_effect=shutil.copy2), patch('install.write_receipt', side_effect=fail_once):
                with self.assertRaisesRegex(OSError, 'simulated receipt failure'):
                    installer.install(stage, prefix)
            self.assertFalse(new.exists())
            self.assertEqual(old.read_bytes(), b'previous app')
            self.assertEqual(backend.read_bytes(), b'previous backend')
            self.assertEqual(installer.read_receipt(prefix), receipt)



class BrandUpgrade(unittest.TestCase):
    def test_legacy_receipt_backend_and_rollback_are_compatible(self):
        import install as installer
        from unittest.mock import patch
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();stage=root/'archive';prefix=root/'prefix'
            (stage/'bin').mkdir(parents=True);(prefix/'bin').mkdir(parents=True)
            files={'bin/herdr-work':b'new app','bin/herdr-work-projects':b'new backend'}
            for name,data in files.items():(stage/name).write_bytes(data)
            (stage/'MANIFEST.json').write_text(json.dumps({'version':'0.1.4','sha':'a'*40,'files':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}))
            binary=prefix/'bin/herdr-work';backend=prefix/'bin/workdesk-projects'
            binary.write_bytes(b'old app');backend.write_bytes(b'old backend')
            previous={'version':'0.1.3','sha':'b'*40,'files':[str(binary),str(backend)]}
            (prefix/'workdesk-install.json').write_text(json.dumps(previous))
            with patch('install.replace',side_effect=shutil.copy2):
                installer.install(stage,prefix)
                self.assertFalse((prefix/'workdesk-install.json').exists())
                self.assertTrue((prefix/'herdr-work-install.json').exists())
                self.assertEqual(backend.read_bytes(),b'new backend')
                self.assertEqual((prefix/'bin/herdr-work-projects').read_bytes(),b'new backend')
                installer.rollback(prefix)
            self.assertEqual(installer.read_receipt(prefix),previous)
            self.assertTrue((prefix/'workdesk-install.json').exists())
            self.assertFalse((prefix/'herdr-work-install.json').exists())
            self.assertFalse((prefix/'bin/herdr-work-projects').exists())
            self.assertEqual(backend.read_bytes(),b'old backend')
            self.assertEqual(binary.read_bytes(),b'old app')

if __name__ == '__main__':
    unittest.main()
