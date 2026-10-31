"""Version-scoped release notes and draft publication boundaries."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release


CHANGELOG = """# Changelog

## Unreleased

- 尚未发布的改动。

## 0.1.15 — 2026-10-31

- 本版改动。

### 修复

- 保留本版子标题。

## 0.1.1 — 2026-10-19

- 更早版本的改动。
"""


class ReleaseNotes(unittest.TestCase):
    def test_notes_only_include_exact_version_and_keep_nested_sections(self):
        notes = release.release_notes(CHANGELOG, 'v0.1.15', 'a' * 40, 'example/project')
        self.assertIn('- 本版改动。\n\n### 修复\n\n- 保留本版子标题。', notes)
        for excluded in ('Unreleased', '尚未发布', '更早版本', '# Changelog', 'HerdrWork v0.1.15'):
            self.assertNotIn(excluded, notes)
        self.assertIn('https://github.com/example/project/blob/v0.1.15/CHANGELOG.md', notes)
        details = notes.split('<details>')[1]
        self.assertIn('Windows', details)
        self.assertIn('SHA256SUMS', details)
        self.assertIn('a' * 40, details)

    def test_last_entry_does_not_match_version_prefix(self):
        self.assertEqual(release.changelog_entry(CHANGELOG, 'v0.1.1'), '- 更早版本的改动。')

    def test_version_heading_can_include_v_prefix_and_prerelease(self):
        changelog = '## v0.2.0-rc.1 — 2026-10-31\n\n- 候选。\n\n## 0.1.15\n\n- 旧版。\n'
        self.assertEqual(release.changelog_entry(changelog, 'v0.2.0-rc.1'), '- 候选。')

    def test_missing_empty_and_duplicate_versions_fail(self):
        for changelog in (CHANGELOG, '## 0.2.0\n\n## 0.1.0\n- old\n',
                          '## 0.2.0\n- first\n\n## 0.2.0\n- second\n'):
            with self.subTest(changelog=changelog):
                with self.assertRaisesRegex(ValueError, 'one non-empty'):
                    release.changelog_entry(changelog, 'v0.2.0')


class PublishNotes(unittest.TestCase):
    def publish_fixture(self, *, existing=None, tag='v0.1.15', changelog=CHANGELOG):
        """Run the real publisher with verified manifests and an in-memory gh service."""
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'CHANGELOG.md').write_text(changelog, encoding='utf-8')
            manifests = {}
            for index, target in enumerate(release.TARGETS):
                archive = root / f'platform-{index}.zip'
                archive.write_bytes(b'archive')
                archive.with_name(archive.name + '.sha256').write_text('checksum')
                manifests[archive.name] = {'product': 'HerdrWork', 'version': tag[1:],
                                          'sha': 'a' * 40, 'target': target}
            calls, notes = [], []

            def gh(command, **kwargs):
                calls.append(command)
                if '--notes-file' in command:
                    notes.append(Path(command[command.index('--notes-file') + 1]).read_text(encoding='utf-8'))
                if command[1:3] == ['release', 'view']:
                    return subprocess.CompletedProcess(command, 0 if existing else 1,
                                                       json.dumps(existing), '')
                return subprocess.CompletedProcess(command, 0, '', '')

            with patch.object(release, 'ROOT', root), \
                 patch.object(release, 'verify', side_effect=lambda path: manifests[path.name]), \
                 patch.object(release.subprocess, 'run', side_effect=gh):
                release.publish(root, tag, 'a' * 40, 'example/project')
            return calls, notes

    def test_new_release_uses_scoped_notes_and_publishes_after_uploads(self):
        calls, notes = self.publish_fixture()
        self.assertEqual(len(notes), 1)
        self.assertNotIn('尚未发布', notes[0])
        self.assertNotIn('更早版本', notes[0])
        create = next(call for call in calls if call[1:3] == ['release', 'create'])
        self.assertIn('--draft', create)
        self.assertEqual(calls[-1][1:5], ['release', 'edit', 'v0.1.15', '--draft=false'])
        self.assertEqual(sum(call[1:3] == ['release', 'upload'] for call in calls), 7)

    def test_resumed_draft_refreshes_notes_before_upload_and_publication(self):
        calls, notes = self.publish_fixture(existing={'isDraft': True, 'assets': []})
        self.assertEqual(len(notes), 1)
        self.assertNotIn('更早版本', notes[0])
        self.assertIn('--notes-file', calls[1])
        self.assertFalse(any(call[1:3] == ['release', 'create'] for call in calls))
        self.assertIn('--draft=false', calls[-1])

    def test_prerelease_keeps_its_publication_flag(self):
        calls, _ = self.publish_fixture(tag='v0.2.0-rc.1', changelog='## 0.2.0-rc.1\n\n- 候选。\n')
        create = next(call for call in calls if call[1:3] == ['release', 'create'])
        self.assertIn('--prerelease', create)

    def test_public_release_is_still_protected_from_overwrite(self):
        with self.assertRaisesRegex(ValueError, 'already public'):
            self.publish_fixture(existing={'isDraft': False, 'assets': []})

    def test_missing_version_fails_before_any_remote_request(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'CHANGELOG.md').write_text(CHANGELOG, encoding='utf-8')
            with patch.object(release, 'ROOT', root), \
                 patch.object(release.subprocess, 'run') as run:
                with self.assertRaisesRegex(ValueError, 'one non-empty'):
                    release.publish(root, 'v99.0.0', 'a' * 40, 'example/project')
                run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
