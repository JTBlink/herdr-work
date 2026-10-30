"""Production Projects packages and archives use the root workspace exclusively."""
from pathlib import Path
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package

ROOT = Path(__file__).resolve().parents[2]
PROJECTS = {
    'herdr-work-projects', 'herdr-work-task-data', 'herdr-work-task-flow',
    'herdr-work-task-board', 'herdr-work-task-web',
}


class ProjectsWorkspace(unittest.TestCase):
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

    def test_all_projects_packages_are_root_members_with_local_sources(self):
        workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']
        for name in PROJECTS:
            relative = f'crates/{name}'
            self.assertIn(relative, workspace['members'])
            project = tomllib.loads((ROOT / relative / 'Cargo.toml').read_text())
            self.assertEqual(project['package']['name'], name)
            self.assertEqual(project['package']['version'], {'workspace': True})
            sections = [project, *project.get('target', {}).values()]
            for section in sections:
                for dependency in section.get('dependencies', {}).values():
                    if isinstance(dependency, dict) and 'path' in dependency:
                        path = (ROOT / relative / dependency['path']).resolve()
                        self.assertTrue(path.is_relative_to(ROOT / 'crates'))
                        self.assertIn(path.relative_to(ROOT).as_posix(), workspace['members'])
        self.assertFalse((ROOT / 'upstream/herdr-projects').exists())

    def test_archive_carries_migrated_projects_license_and_only_product_binaries(self):
        # A real archive exercises the packager's source lookup after the old path is removed.
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = 'aarch64-apple-darwin'
            for name in ['herdr-work', 'herdr-work-projects']:
                path = root / 'target' / target / 'release' / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('compiled fixture')
            for name in ['scripts/install.py', 'scripts/hook.py', 'README.md',
                         'herdr-plugin.toml', 'LICENSE', 'CHANGELOG.md',
                         'crates/herdr-work-projects/LICENSE', 'crates/herdr-work-code/LICENSE',
                         'crates/herdr-work-engine/LICENSE-process-runtime']:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('preserved Projects copyright' if 'projects/LICENSE' in name else 'fixture')
            plan = {'product': 'HerdrWork', 'version': '0.1.0', 'target': target,
                    'sha': 'a' * 40, 'projects_backend': True}
            with patch.object(package, 'ROOT', root), patch.object(package, 'metadata', return_value=plan):
                archive = package.package(target, root / 'dist')
            import tarfile
            with tarfile.open(archive) as tar:
                paths = {Path(m.name).relative_to(tar.getmembers()[0].name).as_posix()
                         for m in tar.getmembers() if m.isfile()}
                self.assertEqual({p for p in paths if p.startswith('bin/')},
                                 {'bin/herdr-work', 'bin/herdr-work-projects'})
                member = next(m for m in tar.getmembers() if m.name.endswith('licenses/herdr-projects.txt'))
                self.assertEqual(tar.extractfile(member).read(), b'preserved Projects copyright')
