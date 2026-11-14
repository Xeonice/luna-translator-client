"""Fail-closed identity, source-scope and historical-debt checks."""
import hashlib
import json
import os
import subprocess
from pathlib import Path

IGNORED_ROOTS = {'.git', '.build', 'target', 'dist', 'reports', 'runtime', 'diagnostics'}


def is_source_path(relative):
    """Generated Tauri staging is restored by the build, not source authority."""
    relative = Path(relative)
    return bool(relative.parts) and not (
        relative.parts[0] in IGNORED_ROOTS
        or relative.name == '.DS_Store'
        or any(part in {'node_modules', '__pycache__'} for part in relative.parts)
        or relative.is_relative_to('apps/desktop/dist')
        or relative.is_relative_to('apps/desktop/src-tauri/resources'))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def safe_path(root, relative):
    root = root.resolve()
    candidate = root / relative
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute() or '..' in Path(relative).parts:
        raise ValueError(f'Unsafe repository path: {relative!r}')
    for parent in [candidate, *candidate.parents]:
        if parent == root:
            break
        if parent.is_symlink():
            raise ValueError(f'Symlink is not valid evidence/source identity: {relative}')
    if not candidate.resolve().is_relative_to(root):
        raise ValueError(f'Path escapes repository: {relative}')
    return candidate


def source_files(root):
    result = {}

    def excluded(relative):
        return not is_source_path(relative)

    # Excluded trees never contribute source identity. Prune them before walking
    # so large build/evidence caches cannot dominate every independent check.
    # Managed symlink directories contribute their raw link identity, not their
    # contents, exactly as Path.rglob's default non-following traversal did.
    for current, directories, files in os.walk(root, topdown=True, followlinks=False):
        current = Path(current)
        retained = []
        for name in directories:
            p = current / name
            relative = p.relative_to(root)
            if excluded(relative):
                continue
            if p.is_symlink():
                result[str(relative)] = 'symlink:' + os.readlink(p)
            else:
                retained.append(name)
        directories[:] = retained
        for name in files:
            p = current / name
            relative = p.relative_to(root)
            if excluded(relative):
                continue
            if p.is_symlink():
                result[str(relative)] = 'symlink:' + os.readlink(p)
            elif p.is_file():
                result[str(relative)] = digest(p)
    return result


def git_commit(root, base):
    result = subprocess.run(['git', 'rev-parse', '--verify', '--end-of-options', str(base) + '^{commit}'], cwd=root, capture_output=True, text=True)
    if result.returncode or len(result.stdout.strip()) not in {40, 64}:
        raise ValueError('Git base must resolve to an actual commit')
    return result.stdout.strip()


def allowed(path, patterns):
    return any(path == pattern or pattern.endswith('/**') and path.startswith(pattern[:-2]) for pattern in patterns)


def changed_sources(root, snapshot, base=None, ci=False):
    git = subprocess.run(['git', 'rev-parse', '--show-toplevel'], cwd=root, capture_output=True, text=True)
    if git.returncode == 0:
        if not base:
            if ci:
                raise ValueError('CI requires an explicit Git base; a change-owned snapshot cannot replace base diff')
            base = 'HEAD'
        # Resolve a commit first: values such as --output=... must never become git options.
        commit = git_commit(root, base)
        # NUL output preserves spaces/newlines; include both sides of renames/deletions.
        result = subprocess.run(['git', 'diff', '--name-only', '--no-renames', '-z', commit, '--'], cwd=root, capture_output=True)
        if result.returncode:
            raise ValueError('Cannot compare Git base: ' + result.stderr.decode(errors='replace'))
        untracked = subprocess.run(['git', 'ls-files', '--others', '--exclude-standard', '-z'], cwd=root, capture_output=True, check=True)
        return sorted({p.decode() for p in (result.stdout + untracked.stdout).split(b'\0') if p})
    if ci:
        raise ValueError('CI may not fall back to a self-declared source snapshot')
    data = json.loads(safe_path(root, snapshot).read_text())
    # Apply the same code-owned generated-output filter to historical maps.
    # This neither edits the frozen snapshot nor hides a managed source path.
    before = {p: identity for p, identity in data['files'].items() if is_source_path(p)}
    current = source_files(root)
    for relative in before:
        safe_path(root, relative)
    return sorted(path for path in before.keys() | current.keys() if before.get(path) != current.get(path))


def check_scope(root, metadata, base=None, ci=False):
    paths = changed_sources(root, metadata['source_snapshot'], base, ci)
    errors = [f'Undeclared source change: {p}' for p in paths if not allowed(p, metadata['allowed_paths'])]
    for relative in paths:
        candidate = root / relative
        if candidate.is_symlink():
            errors.append('Source symlink is not an allowed implementation path: ' + relative)
    return paths, errors


def check_baseline(root):
    data = json.loads(safe_path(root, 'docs/development/iteration-baseline.json').read_text())
    errors = []
    paths = set()
    for entry in data['files']:
        path = safe_path(root, entry['path'])
        if entry['path'] in paths:
            errors.append('Duplicate baseline path: ' + entry['path'])
        paths.add(entry['path'])
        if not path.is_file() or digest(path) != entry['sha256']:
            errors.append('Authority baseline drift: ' + entry['path'])
    if not paths:
        errors.append('Authority baseline is empty')
    return errors


def finding_key(finding):
    return json.dumps({key: finding[key] for key in ['path', 'rule', 'evidence']}, sort_keys=True, ensure_ascii=False)


def check_debt(root, findings, release=False, base=None, ci=False):
    data = json.loads(safe_path(root, 'openspec/legacy-debt.json').read_text())
    git = subprocess.run(['git', 'rev-parse', '--show-toplevel'], cwd=root, capture_output=True, text=True)
    if git.returncode == 0:
        if ci and not base:
            return [], ['CI debt comparison requires an explicit base commit']
        commit = git_commit(root, base or 'HEAD')
        old = subprocess.run(['git', 'show', commit + ':openspec/legacy-debt.json'], cwd=root, capture_output=True, text=True)
        if old.returncode:
            return [], ['Trusted base debt ledger is missing; bootstrap must be reviewed before it becomes a baseline']
        trusted = json.loads(old.stdout)
    else:
        baseline = json.loads(safe_path(root, 'docs/development/iteration-baseline.json').read_text())
        entries = [entry for entry in baseline['files'] if entry['path'] == 'openspec/legacy-debt.json']
        if len(entries) != 1 or digest(root / 'openspec/legacy-debt.json') != entries[0]['sha256']:
            return [], ['Debt ledger differs from its trusted authority fingerprint']
        trusted = data
    known = {finding_key(row): row for row in trusted['findings']}
    active = {finding_key(row): row for row in data['findings']}
    if any(key not in known or row['source_sha256'] != known[key]['source_sha256'] for key, row in active.items()):
        return [], ['Debt ledger may shrink, but new findings or updated source hashes may not self-authorize']
    if len(active) != len(data['findings']) or len(known) != len(trusted['findings']):
        return [], ['Historical debt contains duplicate findings']
    tolerated, errors = [], []
    for finding in findings:
        key = finding_key(finding)
        path = safe_path(root, finding['path'])
        entry = active.get(key)
        if not release and entry and path.is_file() and digest(path) == entry['source_sha256']:
            tolerated.append(finding)
        else:
            errors.append(f'{finding["rule"]}: {finding["path"]}: {finding["evidence"]}')
    return tolerated, errors
