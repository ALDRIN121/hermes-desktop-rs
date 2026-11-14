import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / 'scripts' / 'import_renderer.py'


class RendererImportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        self.git('init', '-q')
        for name in ['apps/desktop/src/main.tsx', 'apps/shared/src/index.ts',
                     'tests/fixtures/session-resume-active-turn.json',
                     'package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml',
                     'pm/lock.json', 'LICENSE', '.npmrc',
                     'hermes_cli/config_defaults.py', 'hermes_cli/webapp_previews.py',
                     'tools/bot_relay.py', 'tui_gateway/contracts/canonical_projections.py',
                     'scripts/build/freshness.mjs', 'scripts/generate-icons.mjs',
                     'scripts/generate_icons.py', 'assets/icon.svg', 'pyproject.toml', 'uv.lock', 'eslint.config.shared.mjs']:
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('tracked original\n')
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 'commit', '-qm', 'fixture')
        self.revision = self.git('rev-parse', 'HEAD').strip()

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.source), *args], text=True)

    def run_import(self, *args):
        return subprocess.run(['python3', str(SCRIPT), '--source', str(self.source),
                               '--destination', str(self.root / 'snapshot'),
                               '--revision', self.revision, *args], capture_output=True, text=True)

    def test_imports_pinned_git_content_not_user_state_or_dirty_files(self):
        (self.source / '.env').write_text('DO_NOT_COPY=private')
        (self.source / 'apps/desktop/src/main.tsx').write_text('uncommitted edit')
        result = self.run_import()
        self.assertEqual(result.returncode, 0, result.stderr)
        snapshot = self.root / 'snapshot'
        self.assertEqual((snapshot / 'apps/desktop/src/main.tsx').read_text(), 'tracked original\n')
        self.assertFalse((snapshot / '.env').exists())
        manifest = json.loads((snapshot / 'UPSTREAM_SNAPSHOT.json').read_text())
        self.assertEqual(manifest['revision'], self.revision)
        self.assertIn('apps/shared/src/index.ts', manifest['files'])
        self.assertIn('tests/fixtures/session-resume-active-turn.json', manifest['files'])

    def test_refuses_to_overwrite_an_existing_snapshot(self):
        self.assertEqual(self.run_import().returncode, 0)
        result = self.run_import()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('already exists', result.stderr)

    def test_check_reports_changed_imported_content(self):
        self.assertEqual(self.run_import().returncode, 0)
        self.assertEqual(self.run_import('--check').returncode, 0)
        (self.root / 'snapshot/apps/desktop/src/main.tsx').write_text('changed')
        result = self.run_import('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('apps/desktop/src/main.tsx', result.stderr)

    def test_recorded_local_changes_are_verified_but_unrecorded_edits_are_not(self):
        self.assertEqual(self.run_import().returncode, 0)
        desktop = self.root / 'snapshot/apps/desktop/src/main.tsx'
        desktop.write_text('intentional redesign')
        self.assertNotEqual(self.run_import('--check').returncode, 0)
        recorded = self.run_import('--record-local-changes')
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        self.assertEqual(self.run_import('--check').returncode, 0)
        manifest = json.loads((self.root / 'snapshot/UPSTREAM_SNAPSHOT.json').read_text())
        self.assertEqual(manifest['localChanges'], ['apps/desktop/src/main.tsx'])
        # A second, unrecorded edit must still fail: recording does not license drift.
        (self.root / 'snapshot/apps/shared/src/index.ts').write_text('silent corruption')
        result = self.run_import('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('apps/shared/src/index.ts', result.stderr)

    def test_check_rejects_unaccounted_files_beside_the_snapshot(self):
        self.assertEqual(self.run_import().returncode, 0)
        (self.root / 'snapshot/apps/desktop/src/stray.ts').write_text('not from the commit')
        result = self.run_import('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unaccounted', result.stderr)
        self.assertIn('apps/desktop/src/stray.ts', result.stderr)

    def test_a_recorded_local_file_may_not_be_deleted(self):
        self.assertEqual(self.run_import().returncode, 0)
        desktop = self.root / 'snapshot/apps/desktop/src/main.tsx'
        desktop.write_text('intentional redesign')
        self.assertEqual(self.run_import('--record-local-changes').returncode, 0)
        desktop.unlink()
        result = self.run_import('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('missing', result.stderr)

    def test_requires_an_immutable_commit_id(self):
        result = self.run_import('--revision', 'HEAD')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('40-character', result.stderr)


if __name__ == '__main__':
    unittest.main()
