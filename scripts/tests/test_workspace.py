"""Production packages and archives use the root workspace exclusively."""
from pathlib import Path
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package

ROOT = Path(__file__).resolve().parents[2]
class ProductWorkspace(unittest.TestCase):
    def test_every_local_production_dependency_stays_in_crates(self):
        workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']
        crates = (ROOT / 'crates').resolve()
        for relative in workspace['members']:
            manifest = (ROOT / relative / 'Cargo.toml').resolve()
            self.assertTrue(manifest.is_relative_to(crates))
            project = tomllib.loads(manifest.read_text())
            for section in [project, *project.get('target', {}).values()]:
                for dependency in section.get('dependencies', {}).values():
                    if isinstance(dependency, dict) and 'path' in dependency:
                        self.assertTrue((manifest.parent / dependency['path']).resolve().is_relative_to(crates))

    def test_workspace_contains_only_the_remaining_product_modules(self):
        workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']
        self.assertEqual(set(workspace['members']), {
            'crates/herdr-work', 'crates/herdr-work-core', 'crates/herdr-work-code',
            'crates/herdr-work-engine', 'crates/herdr-work-tui',
        })

    def test_archive_carries_sidebar_license_and_only_product_binary(self):
        # A real archive exercises the packager's source lookup after the old path is removed.
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = 'aarch64-apple-darwin'
            for name in ['herdr-work']:
                path = root / 'target' / target / 'release' / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('compiled fixture')
            for name in ['scripts/install.py', 'README.md',
                         'herdr-plugin.toml', 'LICENSE', 'CHANGELOG.md',
                         'docs/verification/assets/workbench.png',
                         'docs/verification/assets/sidebar-width-095.png',
                         'crates/herdr-work-code/LICENSE',
                         'crates/herdr-work-engine/LICENSE-process-runtime']:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('preserved Sidebar copyright' if 'herdr-work-code/LICENSE' in name else 'fixture')
            plan = {'product': 'HerdrWork', 'version': '0.1.0', 'target': target,
                    'sha': 'a' * 40}
            with patch.object(package, 'ROOT', root), patch.object(package, 'metadata', return_value=plan):
                archive = package.package(target, root / 'dist')
            import tarfile
            with tarfile.open(archive) as tar:
                paths = {Path(m.name).relative_to(tar.getmembers()[0].name).as_posix()
                         for m in tar.getmembers() if m.isfile()}
                self.assertEqual({p for p in paths if p.startswith('bin/')},
                                 {'bin/herdr-work'})
                self.assertIn('docs/verification/assets/sidebar-width-095.png', paths)
                member = next(m for m in tar.getmembers() if m.name.endswith('licenses/herdr-sidebar.txt'))
                self.assertEqual(tar.extractfile(member).read(), b'preserved Sidebar copyright')
