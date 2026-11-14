#!/usr/bin/env python3
"""Compare a real, read-only SQLite schema with an explicitly frozen design contract.

This verifier never executes production migrations and never treats a design-DDL
clone as proof that the product migration ran. That provenance belongs to the
registered storage adapter and its actual compile/run receipt.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import sqlite3

from database_design import SOURCES, ROOT, _authorizer

CONTRACT = 'docs/architecture/database/schema-contract.json'
# Preserve quoted literals/identifiers exactly; ignore comments, whitespace and
# unquoted identifier/keyword case. This is deliberately conservative: it does
# not attempt to prove arbitrary SQL programs semantically equivalent.
_TOKEN = re.compile(r"\s+|--[^\n]*(?:\n|$)|/\*[\s\S]*?\*/|'(?:''|[^'])*'|\"(?:\"\"|[^\"])*\"|`(?:``|[^`])*`|\[[^\]]*\]|[A-Za-z_][A-Za-z_0-9]*|(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|\|\||<<|>>|<=|>=|<>|!=|==|[^\s]", re.M)


def sql_tokens(sql):
    if sql is None:
        return None
    result = []
    for match in _TOKEN.finditer(sql):
        token = match.group()
        if token.isspace() or token.startswith('--') or token.startswith('/*'):
            continue
        result.append(token.lower() if re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', token) else token)
    while result and result[-1] == ';':
        result.pop()
    return result


def _name(value):
    return '"' + value.replace('"', '""') + '"'


def inspect_connection(connection):
    """Read actual sqlite_schema/PRAGMA objects, including otherwise hidden details."""
    connection.row_factory = sqlite3.Row
    rows = connection.execute("SELECT name,sql FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").fetchall()
    flags = {r['name']: r for r in connection.execute('PRAGMA table_list') if r['schema'] == 'main'}
    tables = {}
    for row in rows:
        name = row['name']
        fks = {}
        for key in connection.execute(f'PRAGMA foreign_key_list({_name(name)})'):
            fk = fks.setdefault(key['id'], {'columns': [], 'parent': key['table'], 'parent_columns': [], 'on_update': key['on_update'], 'on_delete': key['on_delete'], 'match': key['match']})
            fk['columns'].append(key['from'])
            fk['parent_columns'].append(key['to'])
        indexes = {}
        for index in connection.execute(f'PRAGMA index_list({_name(name)})'):
            sql = connection.execute("SELECT sql FROM sqlite_schema WHERE type='index' AND name=?", (index['name'],)).fetchone()
            indexes[index['name']] = {'unique': bool(index['unique']), 'origin': index['origin'], 'partial': bool(index['partial']), 'columns': [dict(value) for value in connection.execute(f'PRAGMA index_xinfo({_name(index["name"])})')], 'sql_tokens': sql_tokens(sql[0]) if sql else None}
        triggers = {item['name']: sql_tokens(item['sql']) for item in connection.execute("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name", (name,))}
        tables[name] = {'columns': [dict(column) for column in connection.execute(f'PRAGMA table_xinfo({_name(name)})')], 'strict': bool(flags[name]['strict']), 'without_rowid': bool(flags[name]['wr']), 'foreign_keys': sorted(fks.values(), key=lambda fk: json.dumps(fk, sort_keys=True)), 'indexes': dict(sorted(indexes.items())), 'triggers': triggers, 'sql_tokens': sql_tokens(row['sql'])}
        # Default expressions are code; normalize cosmetic case/space too.
        for field in tables[name]['columns']:
            field['dflt_value'] = sql_tokens(field['dflt_value'])
    views = {r['name']: sql_tokens(r['sql']) for r in connection.execute("SELECT name,sql FROM sqlite_schema WHERE type='view' AND name NOT LIKE 'sqlite_%' ORDER BY name")}
    return {'tables': tables, 'views': views}


def design_contract(root):
    root = Path(root)
    databases, hashes = {}, {}
    for name, sources in SOURCES.items():
        db = sqlite3.connect(':memory:')
        db.set_authorizer(_authorizer)
        try:
            db.execute('PRAGMA foreign_keys=ON')
            for relative in sources:
                data = (root / relative).read_bytes()
                hashes[relative] = hashlib.sha256(data).hexdigest()
                db.executescript(data.decode('utf-8'))
            databases[name] = inspect_connection(db)
        finally:
            db.close()
    return {'schema_version': 1, 'kind': 'frozen_storage_schema_contract', 'phase': 'candidate_design_pending_review', 'normalization': 'sql_tokens_v1_conservative_no_literal_rewrite', 'ddl_source_sha256': hashes, 'databases': databases, 'production_migration_verified': False}


def _diff(expected, actual, location='schema'):
    errors = []
    if type(expected) is not type(actual):
        return [f'{location}: type differs ({type(expected).__name__} vs {type(actual).__name__})']
    if isinstance(expected, dict):
        errors += [f'{location}.{key}: missing' for key in sorted(expected.keys() - actual.keys())]
        errors += [f'{location}.{key}: unexpected' for key in sorted(actual.keys() - expected.keys())]
        for key in sorted(expected.keys() & actual.keys()):
            errors.extend(_diff(expected[key], actual[key], f'{location}.{key}'))
    elif isinstance(expected, list):
        if len(expected) != len(actual):
            errors.append(f'{location}: length differs ({len(expected)} vs {len(actual)})')
        for index, (left, right) in enumerate(zip(expected, actual)):
            errors.extend(_diff(left, right, f'{location}[{index}]'))
    elif expected != actual:
        errors.append(f'{location}: expected {str(expected)[:160]!r}, actual {str(actual)[:160]!r}')
    return errors


def _read_contract(root, contract_path=CONTRACT):
    root = Path(root).resolve()
    relative = Path(contract_path)
    if relative.is_absolute() or '..' in relative.parts:
        raise ValueError('contract path must be relative within the repository')
    path = root / relative
    if not path.resolve().is_relative_to(root) or any(parent.is_symlink() for parent in [path, *path.parents]):
        raise ValueError('contract path escapes repository or contains a symbolic link')
    if path.is_symlink():
        raise ValueError('schema contract cannot be a symbolic link')
    value = json.loads(path.read_text())
    if value.get('schema_version') != 1 or value.get('kind') != 'frozen_storage_schema_contract' or set(value.get('databases', {})) != set(SOURCES):
        raise ValueError('invalid frozen schema contract header/database identities')
    return value


def validate_schema_contract(root):
    """Check the frozen expected structure; never silently regenerate it."""
    try:
        expected = _read_contract(root)
        actual = design_contract(root)
        return _diff(expected, actual, 'frozen_contract')
    except (OSError, ValueError, sqlite3.Error) as error:
        return [f'schema contract: {error}']


def compare_schema(root, candidate_path, database):
    """Compare against the current frozen contract without altering the DB."""
    return compare_schema_file(root, candidate_path, database, CONTRACT)


def sealed_snapshot_errors(candidate_path):
    """Common precondition for every immutable candidate reader, including rejections."""
    candidate = Path(candidate_path)
    if not candidate.is_file() or candidate.is_symlink() or any(parent.is_symlink() for parent in candidate.parents):
        return [f'candidate database missing or contains symbolic link: {candidate}']
    for suffix in ('-wal', '-shm'):
        sidecar = Path(str(candidate) + suffix)
        # exists() follows links and is false for dangling links; test the link
        # itself first so neither dangling nor zero-byte targets can bypass us.
        if sidecar.is_symlink() or sidecar.exists() and sidecar.stat().st_size:
            return [f'candidate must be a sealed snapshot without nonempty WAL/SHM or symbolic-link sidecars: {sidecar}']
    return []


def _inspect_sealed_snapshot(candidate_path):
    """Collect schema and FK evidence; neither interpretation may skip safety checks."""
    candidate = Path(candidate_path)
    sealed_errors = sealed_snapshot_errors(candidate)
    if sealed_errors:
        return None, [], sealed_errors
    # SQLite receives a correctly escaped file URI, including '?' and '#'.
    # The sidecar check above is mandatory before immutable mode: otherwise
    # a pending WAL transaction could be silently omitted from inspection.
    uri = candidate.resolve().as_uri() + '?mode=ro&immutable=1'
    db = sqlite3.connect(uri, uri=True)
    db.set_authorizer(_authorizer)
    try:
        db.execute('PRAGMA query_only=ON')
        db.execute('PRAGMA foreign_keys=ON')
        integrity = [r[0] for r in db.execute('PRAGMA integrity_check')]
        if integrity != ['ok']:
            return None, [], [f'candidate integrity_check failed: {integrity[:5]}']
        violations = list(db.execute('PRAGMA foreign_key_check'))
        return inspect_connection(db), violations, []
    finally:
        db.close()


def compare_schema_file(root, candidate_path, database, contract_path):
    """Inspect a sealed, standalone valid candidate against its version contract.

    This is a candidate-artifact inspector, not a live database diagnostic. A
    live WAL/SHM must never be ignored by immutable=1 or checkpointed here.
    All ordinary candidates must have zero FK violations.
    """
    try:
        expected = _read_contract(root, contract_path)
        if database not in expected['databases']:
            return [f'unknown database identity: {database}']
        actual, violations, errors = _inspect_sealed_snapshot(candidate_path)
        if errors:
            return errors
        if violations:
            return [f'candidate has {len(violations)} foreign key violations']
        return _diff(expected['databases'][database], actual, database)
    except (OSError, ValueError, sqlite3.Error) as error:
        return [f'candidate schema inspection failed: {error}']


def compare_rejected_relationship_snapshot(root, candidate_path, database, contract_path):
    """Prove a profile rejection fixture has the exact schema and a real bad FK.

    A successful comparison here certifies only the negative fixture, never a
    usable product database or that the product rejected/preserved it. Those
    assertions remain the storage adapter's responsibility. A healthy snapshot
    is deliberately rejected by this entry point; there is no FK-bypass mode.
    """
    try:
        expected = _read_contract(root, contract_path)
        if database != 'profile':
            return [f'rejected relationship snapshot requires profile database identity: {database}']
        actual, violations, errors = _inspect_sealed_snapshot(candidate_path)
        if errors:
            return errors
        errors = _diff(expected['databases']['profile'], actual, 'profile')
        if errors:
            return errors
        if not violations:
            return ['rejected relationship snapshot requires at least one real foreign key violation']
        return []
    except (OSError, ValueError, sqlite3.Error) as error:
        return [f'rejected relationship snapshot inspection failed: {error}']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--check', action='store_true')
    modes.add_argument('--freeze', action='store_true', help='explicit reviewed contract creation/update; never used by verify')
    modes.add_argument('--candidate', type=Path)
    parser.add_argument('--database', choices=tuple(SOURCES))
    args = parser.parse_args()
    if args.freeze:
        value = design_contract(args.root)
        (args.root / CONTRACT).write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + '\n')
        errors = []
    elif args.candidate:
        if not args.database:
            parser.error('--candidate requires --database')
        errors = validate_schema_contract(args.root) or compare_schema(args.root, args.candidate, args.database)
    else:
        errors = validate_schema_contract(args.root)
    print(json.dumps({'status': 'failed' if errors else 'passed', 'scope': 'schema_contract_comparison_only', 'product_migration_executed': False, 'errors': errors}, ensure_ascii=False))
    return 1 if errors else 0


if __name__ == '__main__':
    raise SystemExit(main())
