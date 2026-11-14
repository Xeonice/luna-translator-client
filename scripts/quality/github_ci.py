#!/usr/bin/env python3
"""Restore pinned CI inputs and execute the original real production adapters."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import sys
import tempfile
import urllib.parse
import urllib.request
import unicodedata
import uuid
import zipfile

from gate_core import safe_path
from gate_policy import atomic_write_json, load_context, read_json, snapshot_inputs

ROOT = Path(__file__).resolve().parents[2]
LARGE_RESOURCES = (
    'resources/language/dictionary.sqlite3',
    'resources/language/unidic/matrix.bin',
    'resources/language/unidic/sys.dic',
)
EVIDENCE_PREFIX = '.build/quality/github-activation-20261115/'


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def archive(root, paths, output):
    """Reproducible transport; original evidence bytes are never rewritten."""
    root, output = Path(root).resolve(), Path(output).resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        raise ValueError('archive output already exists')
    entries = {}
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6,
                         allowZip64=True) as bundle:
        for relative in sorted(set(paths)):
            source = safe_path(root, relative)
            if not source.is_file():
                raise ValueError('archive input is not a regular file: ' + relative)
            info = zipfile.ZipInfo(relative, date_time=(2020, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            with source.open('rb') as reader, bundle.open(info, 'w', force_zip64=True) as writer:
                shutil.copyfileobj(reader, writer, 1024 * 1024)
            entries[relative] = {'sha256': sha(source), 'size': source.stat().st_size}
    return {'sha256': sha(output), 'size': output.stat().st_size, 'files': entries}


def download(url, identity, destination):
    parsed = urllib.parse.urlparse(url)
    if (parsed.scheme != 'https' or parsed.netloc != 'github.com'
            or not parsed.path.startswith('/Xeonice/luna-translator-client/releases/download/')):
        raise ValueError('CI input must be a repository-owned HTTPS release asset')
    if not isinstance(identity, str) or len(identity) != 64 or any(c not in '0123456789abcdef' for c in identity):
        raise ValueError('CI input SHA256 is required')
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.is_file() and sha(destination) == identity:
        return destination
    partial = destination.with_suffix('.download-' + uuid.uuid4().hex)
    try:
        request = urllib.request.Request(url, headers={'User-Agent': 'Luna-pinned-CI-inputs/1'})
        with urllib.request.urlopen(request, timeout=120) as response, partial.open('wb') as writer:
            shutil.copyfileobj(response, writer, 1024 * 1024)
        if sha(partial) != identity:
            raise ValueError('CI input archive hash mismatch')
        os.replace(partial, destination)
    finally:
        partial.unlink(missing_ok=True)
    return destination


def restore(root, bundle, *, expected=None, prefix=None):
    """Validate the complete archive before changing the working tree."""
    root = Path(root).resolve()
    with zipfile.ZipFile(bundle) as archive_file:
        rows = archive_file.infolist()
        names = [row.filename for row in rows]
        destinations = {unicodedata.normalize('NFC', PurePosixPath(name).as_posix()).casefold() for name in names}
        if len(names) != len(set(names)) or len(names) != len(destinations) or not names or len(names) > 16384:
            raise ValueError('CI archive has duplicate, empty or excessive inventory')
        if expected is not None and set(names) != set(expected):
            raise ValueError('CI archive inventory differs from its fixed manifest')
        if expected is None and (sum(row.file_size for row in rows) > 512 * 1024 * 1024
                                 or any(row.file_size > 64 * 1024 * 1024 for row in rows)):
            raise ValueError('CI evidence archive exceeds its bounded transport size')
        for row in rows:
            relative = row.filename
            parts = PurePosixPath(relative).parts
            mode = row.external_attr >> 16
            if (not parts or relative != PurePosixPath(relative).as_posix()
                    or relative.startswith('/') or '..' in parts or '\\' in relative
                    or row.is_dir() or stat.S_IFMT(mode) not in {0, stat.S_IFREG}
                    or prefix is not None and not relative.startswith(prefix)):
                raise ValueError('CI archive unsafe member: ' + relative)
            safe_path(root, relative)
            if expected is not None and row.file_size != expected[relative]['size']:
                raise ValueError('CI archive file size mismatch: ' + relative)
        # Stage all content, check actual bytes, then install. No rejected
        # member may leave a partly restored source/authority tree behind.
        with tempfile.TemporaryDirectory(prefix='luna-ci-restore-') as folder:
            staged = Path(folder)
            for row in rows:
                target = staged / row.filename
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive_file.open(row) as reader, target.open('wb') as writer:
                    shutil.copyfileobj(reader, writer, 1024 * 1024)
                if expected is not None and sha(target) != expected[row.filename]['sha256']:
                    raise ValueError('CI archive file hash mismatch: ' + row.filename)
            for row in rows:
                target = safe_path(root, row.filename)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(staged / row.filename, target)
    return names


def restore_resources(root, lock):
    expected = lock.get('files')
    if not isinstance(expected, dict) or set(expected) != set(LARGE_RESOURCES):
        raise ValueError('CI language resource closure must contain the three original large files')
    manifest = read_json(root / 'resources/language/manifest.json')
    pinned = {'resources/' + row['path']: {'sha256': row['sha256'], 'size': row['size']}
              for row in manifest['files']}
    if any(expected[path] != pinned[path] for path in expected):
        raise ValueError('CI resource lock differs from the existing product language manifest')
    if all((root / p).is_file() and sha(root / p) == v['sha256'] for p, v in expected.items()):
        return {'status': 'passed', 'restored': False, 'files': len(expected)}
    bundle = download(lock['url'], lock['sha256'], root / '.build/quality/ci-downloads/language.zip')
    restore(root, bundle, expected=expected)
    return {'status': 'passed', 'restored': True, 'files': len(expected)}


def stage_compiler_resources(root):
    """Stage actual pinned language bytes for Tauri's resource-aware compiler.

    This is not the application packaging entry point: Core/broker/worker
    packaging and real-game qualification remain separate mandatory gates.
    """
    root = Path(root).resolve()
    manifest_path = root / 'resources/language/manifest.json'
    manifest = read_json(manifest_path)
    entries = {}
    for entry in manifest['files']:
        relative = entry['path']
        if not relative.startswith(('language/', 'licenses/')) or relative in entries:
            raise ValueError('compiler resource inventory is invalid')
        source = safe_path(root, 'resources/' + relative)
        if (not source.is_file() or source.stat().st_size != entry['size']
                or sha(source) != entry['sha256']):
            raise ValueError('compiler resource differs from product manifest: ' + relative)
        entries[relative] = source
    if not entries:
        raise ValueError('compiler resource inventory is empty')
    entries['language/manifest.json'] = manifest_path
    for relative, source in entries.items():
        target = safe_path(root, 'apps/desktop/src-tauri/resources/' + relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        # APFS clone is an independent copy on write, not a mutable hard link.
        if sys.platform == 'darwin' and not target.exists():
            copied = subprocess.run(['cp', '-c', str(source), str(target)], capture_output=True)
            if copied.returncode == 0:
                continue
        shutil.copyfile(source, target)
    return {'status': 'passed', 'staged_files': len(entries),
            'application_runtime_qualified': False, 'product_acceptance_promoted': False}


@contextmanager
def _plain_adapter_output():
    """Keep real adapter service URLs free of inherited terminal decoration."""
    keys = ('NO_COLOR', 'FORCE_COLOR')
    previous = {key: os.environ.get(key) for key in keys}
    os.environ['NO_COLOR'] = '1'
    os.environ.pop('FORCE_COLOR', None)
    try:
        yield
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def produce(root):
    from development_gates import execute
    root = Path(root).resolve()
    context = load_context(root)
    run = 'github-production-' + os.environ.get('GITHUB_RUN_ID', uuid.uuid4().hex) + '-' + os.environ.get('GITHUB_RUN_ATTEMPT', '1')
    inputs = snapshot_inputs(root)
    mapping = {'functional-component': ('functional-component', 'G06'),
               'functional-timing': ('affected-functional-tests', 'G06'),
               'storage-migration': ('storage-migration', 'G07'),
               'storage-recovery': ('storage-runtime', 'G07')}
    proof = read_json(root / '.github/ci-evidence.lock.json')['proof_path']
    safe_path(root, proof)
    if not proof.startswith('.build/quality/'):
        raise ValueError('real observer must be an isolated quality artifact')
    result = {'schema_version': 1, 'hooks': proof, 'items': {}}
    with _plain_adapter_output():
        for key, (check, gate) in mapping.items():
            receipt = execute(root, context['change_id'], run, check, gate, inputs)
            if receipt['exit_code'] or receipt['test_count'] < 1 or receipt['skipped_count']:
                raise ValueError('actual production adapter did not pass: ' + key)
            result['items'][key] = receipt
    if snapshot_inputs(root) != inputs:
        raise ValueError('production source changed during execution')
    destination = context['effective_gate_external_evidence']
    if not destination.startswith('.build/quality/'):
        raise ValueError('production evidence output must remain an isolated quality artifact')
    atomic_write_json(root, destination, result)
    return {'status': 'passed', 'adapters': len(mapping), 'test_count': sum(r['test_count'] for r in result['items'].values())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['resources', 'compiler-resources', 'evidence', 'review', 'produce', 'pack-resources'])
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    if args.action == 'resources':
        result = restore_resources(root, read_json(root / '.github/ci-resource-lock.json'))
    elif args.action == 'compiler-resources':
        result = stage_compiler_resources(root)
    elif args.action == 'pack-resources':
        if args.output is None:
            parser.error('--output required')
        result = archive(root, LARGE_RESOURCES, args.output)
    elif args.action in {'evidence', 'review'}:
        if args.action == 'evidence':
            lock = read_json(root / '.github/ci-evidence.lock.json')
        else:
            lock = {'url': os.environ.get('GATE_REVIEW_BUNDLE_URL'),
                    'sha256': os.environ.get('GATE_REVIEW_BUNDLE_SHA256')}
        bundle = download(lock['url'], lock['sha256'], root / ('.build/quality/ci-downloads/' + args.action + '.zip'))
        result = {'status': 'passed', 'files': len(restore(root, bundle, prefix=EVIDENCE_PREFIX))}
    else:
        result = produce(root)
    print(json.dumps(result, ensure_ascii=False))


if __name__ == '__main__':
    main()
