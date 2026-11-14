#!/usr/bin/env python3
"""Registered adapters for existing checks. No work-item supplied shell commands."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

from gate_core import check_debt, check_scope as raw_check_scope, digest, safe_path, source_files
from spec_contract import validate_change
from technology_contract import check as technology_check

ROOT = Path(__file__).resolve().parents[2]
RELEASE_CHECKS = frozenset({'artifact-package-check', 'real-game-regression', 'real-translation-service'})


def release_input_identity(root, metadata):
    """Hash the code-owned release input closure, including referenced raw data.

    Source identity deliberately excludes generated files. Release receipts
    therefore need this separate closure. The four roots are fixed here, not
    selected by a receipt; JSON references are followed transitively. Existing
    repository paths in a producer document are included even when that
    producer spells its reference as a string instead of a path/SHA object.
    Directory inputs include their inventory so additions/deletions also age
    the receipt. Limits fail closed instead of silently truncating the map.
    """
    root = Path(root).resolve()
    release = metadata.get('release_inputs')
    if not isinstance(release, dict) or not release.get('artifact'):
        return {}
    result, pending, seen = {}, [], set()

    def enqueue(relative, *, required=True):
        if not isinstance(relative, str) or not relative:
            raise ValueError('release.input_path_invalid')
        path = safe_path(root, relative)
        if not path.exists():
            if required:
                raise ValueError('release.input_missing: ' + relative)
            return
        pending.append(path)

    def references(value):
        if isinstance(value, dict):
            # The standard evidence reference is always a required input.
            if isinstance(value.get('path'), str) and isinstance(value.get('sha256'), str):
                enqueue(value['path'])
            for item in value.values():
                references(item)
        elif isinstance(value, list):
            for item in value:
                references(item)
        elif isinstance(value, str) and value and not Path(value).is_absolute() and '..' not in Path(value).parts:
            # Producer formats also contain root-relative stdout, snapshot,
            # directory, archive and log strings without the standard shape.
            candidate = root / value
            if candidate.is_file() or candidate.is_dir():
                enqueue(value)

    for field in ('artifact', 'evidence', 'native_evidence', 'service_evidence'):
        if release.get(field):
            enqueue(release[field])
    artifact = safe_path(root, release['artifact'])
    enqueue((artifact.parent / 'build-manifest.json').relative_to(root).as_posix())
    while pending:
        path = pending.pop()
        relative = path.relative_to(root).as_posix()
        if relative in seen:
            continue
        seen.add(relative)
        if len(seen) > 16384:
            raise ValueError('release.input_closure_too_large')
        safe_path(root, relative)  # Check every descendant; never follow links.
        if path.is_dir():
            pending.extend(sorted(path.iterdir()))
            # Empty directories and directory/file replacement remain bound.
            result[relative + '/'] = 'directory'
            continue
        if not path.is_file():
            raise ValueError('release.input_not_regular: ' + relative)
        with path.open('rb') as stream:
            result[relative] = hashlib.file_digest(stream, 'sha256').hexdigest()
        if path.suffix == '.json':
            if path.stat().st_size > 16 * 1024 * 1024:
                raise ValueError('release.input_json_too_large: ' + relative)
            references(json.loads(path.read_text(encoding='utf-8')))
    return dict(sorted(result.items()))


def validate_release_input_identity(root, metadata, recorded):
    """Recompute external inputs for passive assessment; never trust a subset."""
    try:
        current = release_input_identity(root, metadata)
    except (ValueError, OSError, TypeError, KeyError) as exc:
        return ['release.input_identity_unavailable: ' + str(exc)]
    if not isinstance(recorded, dict) or recorded != current:
        return ['release.external_inputs_changed']
    return []


def verified_scope(root, metadata, base=None, ci=False):
    """One scope decision shared by the CLI and passive Stop assessment."""
    paths, errors = raw_check_scope(root, metadata, base, ci=ci)
    authority_ref = metadata.get('scope_authority')
    if metadata.get('engine') == 'effective-gates/v1':
        if not isinstance(authority_ref, dict):
            errors.append('scope.snapshot_authority_missing')
        else:
            authority_path = safe_path(root, authority_ref.get('path'))
            if digest(authority_path) != authority_ref.get('sha256'):
                errors.append('scope.snapshot_authority_changed')
            authority = json.loads(authority_path.read_text())
            frozen = authority.get('scope_snapshot', {})
            if (frozen.get('path') != metadata.get('source_snapshot')
                    or digest(safe_path(root, metadata['source_snapshot'])) != frozen.get('sha256')):
                errors.append('scope.snapshot_changed_after_authorization')
    return paths, errors


def check(root, name, change, *, base=None, release=False):
    root = Path(root).resolve()
    errors, facts = [], {}
    if name == 'spec-contract':
        errors = validate_change(root, change)
        metadata = json.loads((root / 'openspec/changes' / change / 'verification.json').read_text())
        if metadata.get('engine') != 'effective-gates/v1':
            errors.append('protocol.effective_gate_migration_required')
    elif name == 'source-scope':
        metadata = json.loads((root / 'openspec/changes' / change / 'verification.json').read_text())
        paths, errors = verified_scope(root, metadata, base, ci=os.environ.get('CI', '').lower() not in {'', '0', 'false', 'no', 'off'})
        facts['actual_changed_paths'] = paths
    elif name == 'technology-contract':
        errors = technology_check(root)
    elif name in {'frontend-boundaries', 'rust-boundaries'}:
        argv = (['node', str(ROOT / 'scripts/quality/frontend-boundaries.mjs')] if name.startswith('frontend')
                else [sys.executable, str(ROOT / 'scripts/quality/rust-boundaries.py')])
        result = subprocess.run([*argv, '--root', str(root), '--json'], cwd=root,
                                capture_output=True, text=True, timeout=180)
        try:
            raw = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise ValueError('checker.invalid_output: ' + result.stderr[-1000:]) from exc
        findings = raw.get('findings')
        if not isinstance(findings, list) or result.returncode not in {0, 1} or bool(findings) != (result.returncode == 1):
            raise ValueError('checker.exit_findings_disagree')
        tolerated, errors = check_debt(root, findings, release=release, base=base,
                                      ci=os.environ.get('CI') == 'true')
        facts.update(findings=findings, frozen_legacy_findings=len(tolerated),
                     product_architecture_conformant=not findings)
    elif name == 'storage-contract':
        from storage_schema import validate_schema_contract
        from storage_boundaries import validate as boundaries_check
        boundary = boundaries_check(root)
        errors = list(validate_schema_contract(root)) + list(boundary.get('errors', []))
        if boundary.get('status') not in {None, 'passed'} and not boundary.get('errors'):
            errors.append('storage.boundary_check_incomplete')
        facts['boundary_status'] = boundary.get('status')
        from storage_schema_v5 import validate_contract as schema_five_contract, assess_boundaries
        errors.extend(schema_five_contract(root))
        five = assess_boundaries(root)
        errors.extend(five.get('errors', []))
        if five.get('status') != 'passed' and not five.get('errors'):
            errors.append('storage.schema_five_boundary_incomplete')
        facts['schema_five'] = five
    elif name in {'artifact-package-check', 'real-game-regression', 'real-translation-service'}:
        metadata = json.loads((root / 'openspec/changes' / change / 'verification.json').read_text())
        release_inputs = metadata.get('release_inputs')
        if not isinstance(release_inputs, dict) or not release_inputs.get('artifact'):
            return {'kind': 'registered-gate-check/v1', 'check_id': name,
                    'status': 'blocked', 'errors': ['release.real_artifact_inputs_missing']}
        artifact = safe_path(root, release_inputs['artifact'])
        if name == 'artifact-package-check':
            from package_identity import inspect_package
            from product_completion import source_identity
            output = root / '.build/quality/effective-gates-package' / uuid.uuid4().hex
            actual = inspect_package(root, artifact, source_identity(root), output)
            facts['package_inspection'] = actual
            if (actual.get('kind') != 'inspected_luna_packaged_build_identity'
                    or actual.get('bundle_resource_signature_verified') is not True):
                errors.append('release.package_inspection_failed')
        else:
            from spec_contract import validate_release
            from verify import required_release_cases
            if not release_inputs.get('evidence'):
                return {'kind': 'registered-gate-check/v1', 'check_id': name,
                        'status': 'blocked', 'errors': ['release.actual_execution_evidence_missing']}
            errors.extend(validate_release(root, release_inputs['evidence'], digest(artifact),
                                           required_release_cases(root)))
            native, service = release_inputs.get('native_evidence'), release_inputs.get('service_evidence')
            if not native or not service:
                return {'kind': 'registered-gate-check/v1', 'check_id': name,
                        'status': 'blocked', 'errors': errors + ['release.actual_native_game_service_producers_missing']}
            # The original real producer/consumer owns fixed Native, game and
            # translation observations. JSON statuses alone cannot qualify it.
            from binding_journey import execute as execute_binding
            safe_path(root, native)
            safe_path(root, service)
            actual = execute_binding(root, '.build/quality/effective-gates-package/' + uuid.uuid4().hex,
                                     nonce=release_inputs.get('nonce'),
                                     native_evidence=native, service_evidence=service)
            facts['actual_binding_execution'] = actual
            if actual.get('status') != 'passed':
                errors.extend(actual.get('errors') or ['release.actual_game_service_execution_incomplete'])
    else:
        raise ValueError('checker.unknown_registered_check: ' + name)
    return {'kind': 'registered-gate-check/v1', 'check_id': name,
            'status': 'failed' if errors else 'passed', 'errors': errors, **facts}


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--check', required=True)
    p.add_argument('--change', required=True)
    p.add_argument('--base')
    p.add_argument('--release', action='store_true')
    args = p.parse_args(argv)
    try:
        result = check(args.root, args.check, args.change, base=args.base, release=args.release)
    except (ValueError, OSError, KeyError, TypeError, subprocess.TimeoutExpired) as exc:
        result = {'kind': 'registered-gate-check/v1', 'status': 'failed',
                  'check_id': args.check, 'errors': ['checker.runtime: ' + str(exc)]}
    print(json.dumps(result, ensure_ascii=False))
    return 0 if result['status'] == 'passed' else (2 if result['status'] == 'blocked' else 1)


if __name__ == '__main__':
    raise SystemExit(main())
