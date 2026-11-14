#!/usr/bin/env python3
"""Import only tracked renderer sources from an immutable upstream commit."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tempfile


PATHS = ('apps/desktop', 'apps/shared', 'tests/fixtures', 'package.json', 'pnpm-lock.yaml',
         'pnpm-workspace.yaml', 'pm/lock.json', 'LICENSE',
         # UI contract tests read these tracked backend definitions directly.
         'hermes_cli/config_defaults.py', 'hermes_cli/webapp_previews.py',
         'tools/bot_relay.py', 'tui_gateway/contracts/canonical_projections.py',
         # The complete upstream desktop freshness identity, including policy.
         '.npmrc', 'scripts/build', 'scripts/generate-icons.mjs',
         'scripts/generate_icons.py', 'assets', 'pyproject.toml', 'uv.lock',
         'eslint.config.shared.mjs')
MANIFEST = 'UPSTREAM_SNAPSHOT.json'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def drift(destination, manifest):
    """Split the tree into pinned files, recorded local edits, and unaccounted additions."""
    local = list(manifest.get('localChanges', []))
    additions = list(manifest.get('localAdditions', []))
    known = set(manifest['files']) | set(local) | set(additions) | {MANIFEST}
    changed, missing = [], []
    for name, expected in manifest['files'].items():
        path = destination / name
        if name in local:
            if not path.is_file():
                missing.append(name)
        elif not path.is_file() or digest(path.read_bytes()) != expected:
            changed.append(name)
    extra = []
    for path in destination.rglob('*'):
        if not path.is_file():
            continue
        name = path.relative_to(destination).as_posix()
        # Dependency trees and caches are build inputs, never part of the snapshot identity.
        if any(part in ('node_modules', '.git', '__pycache__') for part in PurePosixPath(name).parts):
            continue
        if name not in known:
            extra.append(name)
    return changed, missing, extra


def check(destination, revision):
    manifest = json.loads((destination / MANIFEST).read_text())
    if manifest['revision'] != revision:
        raise ValueError('snapshot revision differs from requested revision')
    local = manifest.get('localChanges', [])
    unimported = [name for name in local if name not in manifest['files']]
    if unimported:
        raise ValueError('local change list names files that were never imported: '
                         + ', '.join(unimported))
    changed, missing, extra = drift(destination, manifest)
    if missing:
        raise ValueError('locally changed files are missing: ' + ', '.join(missing))
    if changed:
        raise ValueError('changed or missing imported files: ' + ', '.join(changed)
                         + ' (record intentional edits with --record-local-changes)')
    if extra:
        raise ValueError('unaccounted files beside the pinned snapshot: ' + ', '.join(sorted(extra)))
    return len(manifest['files']) - len(local), len(local)


def record(destination):
    """Freeze the current intentional edits so --check can tell them from corruption."""
    manifest = json.loads((destination / MANIFEST).read_text())
    local, additions = [], []
    for name, expected in manifest['files'].items():
        path = destination / name
        if path.is_file() and digest(path.read_bytes()) != expected:
            local.append(name)
    known = set(manifest['files'])
    for path in destination.rglob('*'):
        if not path.is_file():
            continue
        name = path.relative_to(destination).as_posix()
        if any(part in ('node_modules', '.git', '__pycache__') for part in PurePosixPath(name).parts):
            continue
        if name not in known and name != MANIFEST:
            additions.append(name)
    manifest['localChanges'] = sorted(local)
    manifest['localAdditions'] = sorted(additions)
    (destination / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    return len(local), len(additions)


def snapshot(source, destination, revision):
    if destination.exists():
        raise ValueError('destination already exists; import into a new directory')
    resolved = subprocess.check_output(
        ['git', '-C', str(source), 'rev-parse', '--verify', revision + '^{commit}'], text=True).strip()
    if resolved != revision:
        raise ValueError('revision does not identify the expected commit')
    archive = subprocess.check_output(['git', '-C', str(source), 'archive', revision, '--', *PATHS])
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Stage beside the destination: validation failures never leave a partial snapshot.
    with tempfile.TemporaryDirectory(dir=destination.parent, prefix='.renderer-import-') as temp:
        stage = Path(temp) / 'tree'
        stage.mkdir()
        files = {}
        with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
            for member in bundle:
                path = PurePosixPath(member.name)
                if path.is_absolute() or '..' in path.parts:
                    raise ValueError('unsafe archive path: ' + member.name)
                if member.isdir():
                    continue
                # Never bring a symlink, executable device, or external dependency tree across.
                if not member.isfile():
                    raise ValueError('unsupported archive entry: ' + member.name)
                if any(part in ('node_modules', '.git', '.env') for part in path.parts):
                    raise ValueError('unexpected private/runtime archive entry: ' + member.name)
                data = bundle.extractfile(member).read()
                target = stage / member.name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                target.chmod(0o755 if member.mode & 0o111 else 0o644)
                files[member.name] = digest(data)
        if not files:
            raise ValueError('renderer archive is empty')
        manifest = {'repository': 'https://github.com/NousResearch/hermes-agent',
                    'revision': revision, 'paths': list(PATHS), 'files': files,
                    'archiveSha256': digest(archive)}
        (stage / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
        stage.rename(destination)
    return len(files)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path)
    parser.add_argument('--destination', type=Path, default=Path('renderer/upstream'))
    parser.add_argument('--revision', required=True)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--record-local-changes', action='store_true')
    args = parser.parse_args()
    try:
        if not re.fullmatch('[0-9a-f]{40}', args.revision):
            raise ValueError('revision must be an immutable 40-character lowercase commit ID')
        if args.record_local_changes:
            local, additions = record(args.destination)
            print(f'Recorded {local} locally changed and {additions} added source files')
        elif args.check:
            verified, local = check(args.destination, args.revision)
            print(f'Verified {verified} pinned source files ({local} recorded local changes)')
        else:
            if args.source is None:
                raise ValueError('--source is required to import a snapshot')
            print(f'Imported {snapshot(args.source, args.destination, args.revision)} pinned source files')
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError, tarfile.TarError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
