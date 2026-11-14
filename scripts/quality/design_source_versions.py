"""Read an independently pinned historical design version; never repair inputs.

Logical catalogue hashes remain the approved identities. The eleven permitted
physical relocations are already-frozen copies of those exact bytes. This does
not approve current design sources, pixels, or product runtime behavior.
"""
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
from types import MappingProxyType
from typing import Mapping

MANIFEST_PATH = 'docs/development/design-source-version-v4.json'
CATALOGUE_PATH = 'docs/design/first-product-v1/acceptance-v4/catalogue.json'
APPROVAL_PATH = 'docs/development/design-approval-20261019.json'
BASELINE_PATH = 'docs/development/iteration-baseline.json'
VERSION_ID = 'reviewed-v4-original-sources-v1'
PURPOSE = 'historical_reference_source_resolution_only'
_DESIGN = 'docs/design/first-product-v1/'
ALLOWED_RELOCATIONS = MappingProxyType({
    _DESIGN + name: _DESIGN + 'acceptance-v4/design-origin/' + name
    for name in ('app.js', 'game-library.js', 'review-gaps.css', 'review-gaps.js', 'README.md',
                 'data.js', 'index.html', 'coverage.csv', 'integration.md', 'manifest.json', 'style-guide.md')
})
_SHA = re.compile(r'[0-9a-f]{64}\Z')


def unique_json(raw):
    if isinstance(raw, bytes):
        raw = raw.decode('utf-8')
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('Duplicate source-version JSON key: ' + key)
            result[key] = value
        return result

    def constant(value):
        raise ValueError('Non-JSON source-version value: ' + value)

    def number(value):
        parsed = float(value)
        # JSON numbers have no separate int/float type in Node. Normalize an
        # exactly representable integral spelling (1.0 / 1e0) consistently;
        # nonintegral or unsafe values still fail the typed schema checks.
        return int(parsed) if parsed.is_integer() and abs(parsed) <= 9007199254740991 else parsed

    return json.loads(raw, object_pairs_hook=pairs, parse_constant=constant, parse_float=number)


def safe_file(root, relative):
    if (not isinstance(relative, str) or not relative or relative != relative.strip()
            or any(ord(c) < 32 or ord(c) == 127 for c in relative)
            or any(c in relative for c in ('\\', ':', '?', '#'))
            or PurePosixPath(relative).is_absolute()
            or any(part in {'', '.', '..'} for part in relative.split('/'))):
        raise ValueError('Unsafe source-version path: ' + str(relative))
    current = root
    for part in relative.split('/'):
        current /= part
        if current.is_symlink():
            raise ValueError('Source-version symlink is forbidden: ' + relative)
    if not current.resolve().is_relative_to(root) or not current.is_file():
        raise ValueError('Missing or escaped source-version file: ' + relative)
    return current


def _hash(raw):
    return hashlib.sha256(raw).hexdigest()


def _pinned_bytes(root, baseline, relative):
    raw = safe_file(root, relative).read_bytes()
    pins = [row for row in baseline.get('files', [])
            if isinstance(row, dict) and row.get('path') == relative]
    if (len(pins) != 1 or pins[0].get('sha256') != _hash(raw)
            or 'bytes' in pins[0] and pins[0]['bytes'] != len(raw)):
        raise ValueError('Source-version input lacks an exact independent authority pin: ' + relative)
    return raw


@dataclass(frozen=True)
class SourceVersion:
    root: Path
    paths: Mapping[str, str]
    receipt: dict

    def resolve(self, logical):
        if logical not in self.paths:
            raise ValueError('Unregistered logical source-version input: ' + str(logical))
        return safe_file(self.root, self.paths[logical])


def load_source_version(root, catalogue=None, catalogue_sha256=None):
    """Verify the entire fixed v4 resolution domain and return read-only paths.

    No map is generated at load time and there is no alternative-source search.
    Callers may pass the catalogue they already inspected; it must be the exact
    independently pinned file, not a replacement object or a proposal profile.
    """
    root = Path(root).absolute()
    if root.is_symlink():
        raise ValueError('Source-version repository root may not be a symlink')
    root = root.resolve()
    baseline = unique_json(safe_file(root, BASELINE_PATH).read_bytes())
    if not isinstance(baseline, dict) or not isinstance(baseline.get('files'), list):
        raise ValueError('Invalid source-version authority baseline')
    manifest_raw = _pinned_bytes(root, baseline, MANIFEST_PATH)
    manifest = unique_json(manifest_raw)
    catalogue_raw = _pinned_bytes(root, baseline, CATALOGUE_PATH)
    actual_catalogue = unique_json(catalogue_raw)
    approval_raw = _pinned_bytes(root, baseline, APPROVAL_PATH)
    approval = unique_json(approval_raw)
    if (not isinstance(actual_catalogue, dict) or actual_catalogue.get('schemaVersion') != 4
            or actual_catalogue.get('status') != 'registered_independently_reviewed'
            or catalogue is not None and catalogue != actual_catalogue
            or catalogue_sha256 is not None and catalogue_sha256 != _hash(catalogue_raw)):
        raise ValueError('Source-version catalogue/profile identity differs')
    catalogue = actual_catalogue
    if (not isinstance(manifest, dict)
            or set(manifest) != {'schema_version', 'version_id', 'purpose', 'catalogue', 'approval', 'entries'}
            or type(manifest['schema_version']) is not int or manifest['schema_version'] != 1
            or manifest['version_id'] != VERSION_ID or manifest['purpose'] != PURPOSE
            or manifest['catalogue'] != {'path': CATALOGUE_PATH, 'sha256': _hash(catalogue_raw)}
            or manifest['approval'] != {'path': APPROVAL_PATH, 'sha256': _hash(approval_raw)}):
        raise ValueError('Source-version manifest version/catalogue/approval contract differs')
    frozen = catalogue.get('frozenFiles')
    approved = approval.get('source_files_sha256') if isinstance(approval, dict) else None
    if (not isinstance(frozen, dict) or not frozen or not isinstance(approved, dict)
            or len(approved) != 16 or catalogue.get('approvedDesignSources') != approved
            or any(frozen.get(p) != h for p, h in approved.items())):
        raise ValueError('Source-version original approved source closure differs')
    entries = manifest['entries']
    if not isinstance(entries, list) or len(entries) != len(frozen):
        raise ValueError('Source-version mapping must cover the exact complete logical domain')
    paths, resolutions, contents = {}, [], {}
    for row in entries:
        if not isinstance(row, dict) or set(row) != {'logical_path', 'physical_path', 'sha256', 'bytes'}:
            raise ValueError('Invalid source-version mapping row')
        logical, physical, expected, size = (row[k] for k in ('logical_path', 'physical_path', 'sha256', 'bytes'))
        if (not isinstance(logical, str) or logical in paths or logical not in frozen
                or not logical.startswith((_DESIGN, 'apps/desktop/tests/visual/'))
                or not isinstance(expected, str) or not _SHA.fullmatch(expected)
                or frozen[logical] != expected or type(size) is not int or size <= 0):
            raise ValueError('Source-version logical identity/domain differs: ' + str(logical))
        # Validate both syntax domains, even when the active logical bytes have
        # changed. Only the explicitly selected physical bytes are historical.
        safe_file(root, logical)
        target = safe_file(root, physical)
        if physical != logical and (ALLOWED_RELOCATIONS.get(logical) != physical
                                    or frozen.get(physical) != expected):
            raise ValueError('Unapproved source-version relocation: ' + logical)
        raw = contents.setdefault(physical, target.read_bytes())
        if _hash(raw) != expected or len(raw) != size:
            raise ValueError('Historical source-version physical bytes differ: ' + logical)
        paths[logical] = physical
        resolutions.append(dict(logical_path=logical, physical_path=physical, sha256=expected, bytes=size))
    if set(paths) != set(frozen) or any(paths.get(p) != p for p in paths.values()):
        raise ValueError('Source-version logical domain or one-level mapping differs')
    active = {p: _hash(safe_file(root, p).read_bytes()) for p in approved}
    return SourceVersion(root, MappingProxyType(paths), {
        'manifestPath': MANIFEST_PATH, 'manifestSha256': _hash(manifest_raw),
        'versionId': VERSION_ID, 'catalogueSha256': _hash(catalogue_raw),
        'approvalSha256': _hash(approval_raw), 'resolutions': resolutions,
        'activeDesignSourceHashes': active,
    })
