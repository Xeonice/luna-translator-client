#!/usr/bin/env python3
"""Codex lifecycle adapter for the project's shared quality policy.

This is a development guardrail, not an OS sandbox.  It never invents a passed
gate receipt, executes user-supplied commands, grants hook trust, or rolls back a
completed tool.  The independent project verification command remains the
delivery authority when a host skips or fails a hook.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import sys
import tempfile
from typing import Any

from gate_core import allowed, safe_path

SUPPORTED_EVENTS = {'SessionStart', 'PreToolUse', 'PostToolUse', 'Stop'}
MAX_INPUT = 1024 * 1024
MAX_CONTINUATIONS = 2
MAX_PENDING_TOOLS = 64
PROBE_FILE_BYTES = 1024 * 1024
PROBE_TOTAL_BYTES = 2 * 1024 * 1024
PROBE_MAX_FILES = 8000
PROBE_HASH_FILES = 128
TRANSIENT_ROOTS = {'.build', '.git', 'target', 'dist', 'reports', 'runtime', 'diagnostics'}
TRANSIENT_PARTS = {'node_modules', '__pycache__'}
PORTABLE_HOOK_COMMAND = ('/usr/bin/env python3 '
                         '"$(git rev-parse --show-toplevel)/scripts/quality/codex_hooks.py" '
                         '--root "$(git rev-parse --show-toplevel)"')


def materialize_hook_command(command: str, execution_source_root: str | Path) -> list[str]:
    """Resolve only our fixed deployment-root expression, without shell execution.

    The observer records the real execution source root. A verifier on another
    checkout can reproduce its argv using that recorded root, while comparing
    the seven actual source hashes separately. This does not normalize handler,
    interpreter, flags, matcher or timeout changes, and accepts no arbitrary
    command substitution. The old host path need not exist on the verifier.
    """
    if command != PORTABLE_HOOK_COMMAND:
        raise ValueError('hook.command_not_portable_contract')
    if not isinstance(execution_source_root, (str, Path)):
        raise ValueError('hook.execution_source_root_invalid')
    text = str(execution_source_root)
    root = Path(text)
    if (not root.is_absolute() or root == Path(root.anchor)
            or text != str(root) or '..' in root.parts
            or any(ord(char) < 32 for char in text)):
        raise ValueError('hook.execution_source_root_invalid')
    return ['/usr/bin/env', 'python3', str(root / 'scripts/quality/codex_hooks.py'), '--root', text]


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _json_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def find_root(cwd: str | Path) -> Path:
    """Resolve the configured project from a nested invocation directory."""
    p = Path(cwd).resolve()
    for candidate in (p, *p.parents):
        if (candidate / '.codex/gate-context.json').is_file():
            return candidate
    raise ValueError('hook.context_missing: no project .codex/gate-context.json')


def _policy():
    # Delayed import keeps error reporting structured if an installation is
    # incomplete.  Missing shared code must not create a synthetic success.
    import gate_policy
    return gate_policy


def _context(root: Path) -> dict:
    context = _policy().load_context(root)
    if not isinstance(context, dict):
        raise ValueError('hook.context_invalid: shared policy returned no context')
    return context


def _snapshot(root: Path, targets: list[str] | None = None) -> dict:
    """Bounded hot-path observation, explicitly not a complete source proof.

    Known explicit edits read just their targets. Opaque shell uses a bounded
    small-file probe; large/unobserved inputs require full project verification.
    No mtime or partial probe can grant a gate pass.
    """
    files: dict[str, str | None] = {}
    unobserved: list[str] = []
    budget = 0
    if targets is None:
        candidates = []
        for current, directories, names in os.walk(root, topdown=True, followlinks=False):
            current_path = Path(current)
            directories[:] = sorted(name for name in directories if name not in TRANSIENT_PARTS
                                      and not (current_path == root and name in TRANSIENT_ROOTS)
                                      and not (current_path == root / 'apps/desktop' and name == 'dist'))
            for name in sorted(names):
                candidates.append((current_path / name).relative_to(root).as_posix())
                if len(candidates) >= PROBE_MAX_FILES:
                    unobserved.append('probe_file_limit')
                    break
            if len(candidates) >= PROBE_MAX_FILES:
                break
    else:
        candidates = sorted(set(targets))
    for index, relative in enumerate(candidates):
        path = root / relative
        if targets is None and index >= PROBE_HASH_FILES:
            files[relative] = 'unobserved'
            unobserved.append(relative)
            continue
        if path.is_symlink():
            files[relative] = 'symlink:' + os.readlink(path)
        elif not path.exists():
            files[relative] = None
        elif path.is_dir():
            files[relative] = 'directory'
        else:
            size = path.stat().st_size
            if size > PROBE_FILE_BYTES or budget + size > PROBE_TOTAL_BYTES:
                files[relative] = 'unobserved'
                unobserved.append(relative)
            else:
                budget += size
                files.update(_policy().snapshot_inputs(root, [relative]))
    return {'kind': 'bounded-hook-probe/v1', 'files': files,
            'bytes_read': budget, 'requires_full_verification': bool(unobserved),
            'unobserved': unobserved, 'inventory_complete': 'probe_file_limit' not in unobserved,
            'targets': targets}


def _changes(root: Path, before: dict) -> tuple[list[str], dict]:
    after = _snapshot(root, before.get('targets'))
    old, new = before['files'], after['files']
    changes = []
    for relative in old.keys() | new.keys():
        if relative not in old or relative not in new:
            if before.get('inventory_complete') and after.get('inventory_complete'):
                changes.append(relative)
        elif 'unobserved' not in {old[relative], new[relative]} and old[relative] != new[relative]:
            changes.append(relative)
    return sorted(changes), after


def _assess(root: Path, context: dict) -> dict:
    from development_gates import assess_current
    return assess_current(root, context, stage='delivery')


def _scope(context: dict) -> list[str]:
    paths = context.get('allowed_paths')
    if not isinstance(paths, list) or not paths or any(not isinstance(x, str) for x in paths):
        raise ValueError('hook.scope_missing: current work item has no allowed paths')
    return paths


def _relative(root: Path, cwd: Path, target: str) -> str:
    """Validate an actual path, rejecting traversal and symlinks before IO."""
    if not target or '\x00' in target:
        raise ValueError('hook.path_invalid')
    lexical = Path(target)
    if '..' in lexical.parts:
        raise ValueError('hook.path_escape: parent traversal is not an allowed write target')
    absolute = lexical if lexical.is_absolute() else cwd / lexical
    try:
        relative = absolute.relative_to(root).as_posix()
    except ValueError as error:
        raise ValueError('hook.path_escape: write target is outside the project') from error
    safe_path(root, relative)
    return relative


def _patch_targets(command: str) -> list[str]:
    return re.findall(r'^\*\*\* (?:Add File|Update File|Delete File|Move to): (.+)$', command, re.MULTILINE)


def _shell_targets(command: str, initial_cwd: Path | None = None) -> tuple[list[str], bool]:
    """Recognize explicit writes and literal cwd changes; opaque code stays partial."""
    try:
        tokens = list(shlex.shlex(command, posix=True, punctuation_chars=';&|<>'))
    except ValueError:
        return [], False
    if not tokens:
        return [], True
    targets: list[str] = []
    certain = not any(x in command for x in ('`', '$(', '${', '\n', '<<'))
    segments: list[list[str]] = [[]]
    for token in tokens:
        if token in {';', '&&', '||', '|', '&'}:
            segments.append([])
        else:
            segments[-1].append(token)
    current_cwd = initial_cwd
    cwd_unknown = False
    readonly = {'pwd', 'ls', 'cat', 'head', 'tail', 'rg', 'grep', 'sed', 'stat', 'file', 'which', 'command', 'true', 'false'}
    def add_target(target):
        if target in {'/dev/null', '&1', '&2'}:
            return
        if cwd_unknown:
            targets.append('__unresolved_explicit_cwd__/write')
        elif current_cwd is None or Path(target).is_absolute():
            targets.append(target)
        else:
            targets.append(str(current_cwd / target))
    for segment in segments:
        if not segment:
            continue
        verb = Path(segment[0]).name
        for i, token in enumerate(segment[:-1]):
            if token in {'>', '>>'}:
                add_target(segment[i + 1])
        plain = []
        skip = False
        for token in segment[1:]:
            if skip:
                skip = False
                continue
            if token in {'>', '>>', '<'}:
                skip = True
                continue
            if not token.startswith('-'):
                plain.append(token)
        if verb == 'cd':
            if len(plain) != 1 or any(c in plain[0] for c in ('$', '`', '~')):
                cwd_unknown = True
                certain = False
            elif current_cwd is not None:
                dest = Path(plain[0])
                current_cwd = (dest if dest.is_absolute() else current_cwd / dest).resolve()
                if not current_cwd.is_dir():
                    cwd_unknown = True
                    certain = False
        elif verb in {'touch', 'mkdir', 'rmdir', 'rm', 'unlink', 'truncate', 'tee'}:
            for target in plain:
                add_target(target)
        elif verb in {'cp', 'install'}:
            if len(plain) >= 2:
                add_target(plain[-1])
            else:
                certain = False
        elif verb == 'mv':
            if len(plain) >= 2:
                for target in plain:
                    add_target(target)
            else:
                certain = False
        elif verb in readonly:
            if verb == 'sed' and any(x.startswith('-i') for x in segment):
                certain = False
        elif verb == 'git' and plain and plain[0] in {'status', 'diff', 'log', 'show', 'rev-parse', 'ls-files'}:
            pass
        elif verb in {'echo', 'printf'}:
            pass
        else:
            certain = False
    if targets and any(segment and Path(segment[0]).name == 'cd' for segment in segments) and any(token in {'|', '&', '||'} for token in tokens):
        # A pipeline/background/failed-cd branch has a different shell cwd
        # lifetime. Do not reinterpret it as a sequential authorized write.
        return ['__unresolved_explicit_cwd__/write'], False
    return targets, certain


def write_targets(event: dict, cwd: Path | None = None) -> tuple[list[str], bool]:
    name = str(event.get('tool_name', ''))
    values = event.get('tool_input') or {}
    if not isinstance(values, dict):
        return [], False
    if name in {'apply_patch', 'Edit', 'Write'}:
        command = values.get('command', values.get('patch', ''))
        if isinstance(command, str) and command.startswith('*** Begin Patch'):
            targets = _patch_targets(command)
            return targets, bool(targets)
        path = values.get('file_path', values.get('path'))
        return ([path], True) if isinstance(path, str) else ([], False)
    if name in {'Bash', 'exec_command', 'shell', 'shell_command'}:
        command = values.get('command', values.get('cmd', ''))
        return _shell_targets(command, cwd) if isinstance(command, str) else ([], False)
    lower = name.lower()
    if any(x in lower for x in ('write_file', 'edit_file', 'delete_file', 'move_file', 'create_directory')):
        paths = [values.get(x) for x in ('path', 'file_path', 'source', 'destination')]
        return [x for x in paths if isinstance(x, str)], any(isinstance(x, str) for x in paths)
    return [], False


def _hook_output(event: str, *, context: str | None = None, deny: str | None = None) -> dict:
    result: dict = {}
    if context:
        if event == 'Stop':
            # Stop supports common output/warnings, not additionalContext.
            result['systemMessage'] = context
        else:
            result['hookSpecificOutput'] = {'hookEventName': event, 'additionalContext': context}
    if deny and event == 'PreToolUse':
        result['hookSpecificOutput'] = {'hookEventName': event, 'permissionDecision': 'deny', 'permissionDecisionReason': deny}
    elif deny and event in {'PostToolUse', 'Stop'}:
        result.update({'decision': 'block', 'reason': deny})
    return result


def _store_root(root: Path, session_id: str) -> Path:
    directory = safe_path(root, '.build/quality/codex-hooks/' + _digest(session_id.encode())[:24])
    directory.mkdir(parents=True, exist_ok=True)
    return directory


def _atomic(path: Path, value: dict) -> None:
    descriptor, name = tempfile.mkstemp(prefix='.state-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w') as stream:
            json.dump(value, stream, ensure_ascii=False, sort_keys=True)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


@contextmanager
def _locked(root: Path, event: dict):
    session = str(event.get('session_id', ''))
    if not session or len(session) > 512:
        raise ValueError('hook.session_missing: lifecycle event needs a bounded session id')
    directory = _store_root(root, session)
    lock_path = safe_path(root, (directory / 'state.lock').relative_to(root).as_posix())
    with lock_path.open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        path = safe_path(root, (directory / 'state.json').relative_to(root).as_posix())
        state = json.loads(path.read_text()) if path.is_file() else {'schema_version': 1, 'pending': {}, 'dirty_paths': [], 'continuations': {}}
        yield state, directory
        _atomic(path, state)


def _journal(directory: Path, event: dict, decision: str, **observations) -> None:
    entry = {'schema_version': 1, 'timestamp': _now(), 'event_name': event.get('hook_event_name'),
             'session_id': event.get('session_id'), 'turn_id': event.get('turn_id'),
             'tool_use_id': event.get('tool_use_id'), 'tool_name': event.get('tool_name'), 'decision': decision,
             'observations': observations}
    # Tool arguments/responses/transcripts can contain secrets.  Record only
    # their digest and explicit source paths/derived guard facts.
    if event.get('tool_input') is not None:
        entry['tool_input_sha256'] = _digest(_json_bytes(event['tool_input']))
    journal_path = safe_path(directory, 'events.jsonl')
    with journal_path.open('a') as output:
        output.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + '\n')


def handle_event(root: Path, event: dict) -> dict:
    root = root.resolve()
    name = event.get('hook_event_name')
    if name not in SUPPORTED_EVENTS:
        raise ValueError('hook.unsupported_event')
    cwd = Path(event.get('cwd', str(root))).resolve()
    if not cwd.is_relative_to(root):
        raise ValueError('hook.cwd_escape')
    with _locked(root, event) as (state, directory):
        try:
            context = _context(root)
            scope = _scope(context)
        except (ValueError, OSError, KeyError, TypeError, ImportError) as error:
            reason = 'hook.context_unavailable: ' + str(error)[:500]
            _journal(directory, event, 'context_unavailable', reason=reason)
            if name == 'PreToolUse':
                targets, certain = write_targets(event)
                if targets or not certain:
                    return _hook_output(name, deny=reason)
            return _hook_output(name, context=reason + '; work item remains incomplete. Read-only discussion may continue.')
        change_id = str(context.get('change_id', 'unknown'))
        if state.get('change_id') not in {None, change_id}:
            state.update(pending={}, dirty_paths=[], continuations={})
        state['change_id'] = change_id
        if name == 'SessionStart':
            _journal(directory, event, 'context_loaded', change_id=change_id, allowed_paths=scope)
            return _hook_output(name, context=f'Luna gate context: {change_id}. Writes are limited to the approved work-item paths. Delivery must pass the shared project verifier; manual visual approval is never inferred.')
        if name == 'PreToolUse':
            targets, certain = write_targets(event)
            try:
                values = event.get('tool_input') or {}
                workdir = values.get('workdir', values.get('cwd')) if isinstance(values, dict) else None
                tool_cwd = (Path(workdir) if Path(workdir).is_absolute() else cwd / workdir).resolve() if isinstance(workdir, str) else cwd
                if not tool_cwd.is_relative_to(root) and (targets or not certain):
                    raise ValueError('hook.cwd_escape: write-capable tool working directory is outside the project')
                targets, certain = write_targets(event, tool_cwd)
                if any('__unresolved_explicit_cwd__' in target for target in targets):
                    raise ValueError('hook.compound_cwd_unresolved: explicit write working directory cannot be safely established')
                relative = [_relative(root, tool_cwd, target) for target in targets]
                outside = [p for p in relative if not allowed(p, scope)]
                if outside:
                    raise ValueError('hook.scope_violation: ' + ', '.join(outside))
            except ValueError as error:
                _journal(directory, event, 'deny', reason=str(error))
                return _hook_output(name, deny=str(error))
            tool_id = str(event.get('tool_use_id', ''))
            if not tool_id or len(tool_id) > 512:
                _journal(directory, event, 'deny', reason='hook.tool_id_missing')
                return _hook_output(name, deny='hook.tool_id_missing: cannot correlate before/after source effects')
            observe = targets or not certain
            probe = _snapshot(root, relative if certain else None) if observe else None
            if len(state['pending']) >= MAX_PENDING_TOOLS:
                expired = next(iter(state['pending']))
                state['pending'].pop(expired)
                state['unpaired_tool_result'] = True
                state['last_unpaired_turn_id'] = event.get('turn_id')
                state['requires_full_verification'] = True
                state['last_full_verification_turn_id'] = event.get('turn_id')
                _journal(directory, event, 'pending_observation_evicted', evicted_tool_use_id=expired, incomplete=True)
            state['pending'][tool_id] = {'before': probe, 'targets': relative, 'target_coverage': 'explicit' if certain else 'partial_dynamic_shell'}
            _journal(directory, event, 'allow', targets=relative, target_coverage='explicit' if certain else 'partial_dynamic_shell')
            return _hook_output(name, context=None if certain else 'This dynamic tool operation is not fully predictable before execution. The hook will compare actual source changes; this is not an OS sandbox.')
        if name == 'PostToolUse':
            tool_id = str(event.get('tool_use_id', ''))
            pending = state['pending'].pop(tool_id, None)
            if pending is None:
                state['unpaired_tool_result'] = True
                state['last_unpaired_turn_id'] = event.get('turn_id')
                _journal(directory, event, 'unpaired_result', reason='hook.pre_event_missing')
                return _hook_output(name, context='hook.pre_event_missing: no correlated pre-tool snapshot. Delivery verification is required; this event does not establish a passed check.')
            if pending['before'] is None:
                changed, probe = [], {'kind': 'readonly-tool/no-source-probe', 'requires_full_verification': False}
            else:
                changed, probe = _changes(root, pending['before'])
            if pending['target_coverage'] != 'explicit' or probe['requires_full_verification']:
                state['requires_full_verification'] = True
                state['last_full_verification_turn_id'] = event.get('turn_id')
            state['dirty_paths'] = sorted(set(state.get('dirty_paths', [])) | set(changed))
            if changed:
                state['last_write_turn_id'] = event.get('turn_id')
            impact = _policy().classify_impact(changed)
            outside = [p for p in changed if not allowed(p, scope)]
            _journal(directory, event, 'violation' if outside else 'changes_recorded', changed_paths=changed, outside_scope=outside, impact=impact,
                     probe_kind=probe['kind'], requires_full_verification=bool(state.get('requires_full_verification')))
            if outside:
                state['scope_violation'] = outside
                return _hook_output(name, deny='hook.scope_violation_after_write: ' + ', '.join(outside) + '. The write already happened; delivery is blocked and no rollback is claimed.')
            return _hook_output(name, context='Luna source changes recorded; affected verification is pending. No gate result was fabricated.' if changed else None)
        # Stop is a turn boundary, not automatically a request to close a task.
        if (not state.get('dirty_paths') and not state.get('unpaired_tool_result')
                and not state.get('requires_full_verification')):
            _journal(directory, event, 'readonly_stop')
            return {}
        if (event.get('turn_id') is not None
                and event.get('turn_id') not in {state.get('last_write_turn_id'), state.get('last_unpaired_turn_id'), state.get('last_full_verification_turn_id')}
                and not event.get('stop_hook_active')
                and not context.get('delivery_requested')):
            _journal(directory, event, 'readonly_stop_with_pending_work', incomplete=True)
            return _hook_output(name, context='This read-only turn may end. Earlier modifications remain incomplete until the independent project delivery verifier passes.')
        try:
            result = _assess(root, context)
            if not isinstance(result, dict) or result.get('kind') != 'development-gates-summary/v1':
                raise ValueError('hook.summary_schema_invalid')
            gates = result.get('gates', [])
            if (not isinstance(gates, list) or len(gates) != 9
                    or {row.get('gate_id') for row in gates} != set(_policy().GATE_IDS)
                    or any(row.get('status') not in _policy().STATUSES or type(row.get('required')) is not bool for row in gates)):
                raise ValueError('hook.summary_gates_invalid')
            derived = _policy().build_summary(gates, change_id=context['change_id'])
            if any(result.get(key) != derived[key] for key in ('status', 'incomplete', 'next_stage_allowed')):
                raise ValueError('hook.summary_decision_invalid')
        except Exception as error:
            gates = [_policy().gate_result(gate_id, 'failed' if gate_id == 'G05' else 'not_run', 'hook.assessment_invalid:' + str(error)[:200]) for gate_id in _policy().GATE_IDS]
            result = _policy().build_summary(gates, change_id=context['change_id'])
        unresolved = [row for row in gates if row.get('status') != 'passed' and row.get('required', True)]
        blocked = [row for row in unresolved if row.get('status') == 'blocked']
        actionable = [row for row in unresolved if row.get('status') in {'failed', 'not_run'}]
        if not unresolved:
            # Only the independently recomputed complete project verdict can
            # discharge a missed/partial observation or a repaired violation.
            state.update(dirty_paths=[], continuations={}, requires_full_verification=False,
                         unpaired_tool_result=False)
            state.pop('scope_violation', None)
            _journal(directory, event, 'verified_stop', status=result.get('status'))
            return {}
        if blocked and not actionable:
            _journal(directory, event, 'external_blocked_stop', gate_ids=[row.get('gate_id', row.get('id')) for row in blocked], incomplete=True)
            return _hook_output(name, context='Required external/manual verification is blocked. Report the blocker and incomplete work item; do not repeatedly re-run screenshots or claim delivery complete.')
        fingerprint = _digest(_json_bytes({'dirty': _snapshot(root), 'unresolved': unresolved, 'scope': state.get('scope_violation'), 'unpaired': state.get('unpaired_tool_result')}))
        count = state['continuations'].get(fingerprint, 0)
        if count >= MAX_CONTINUATIONS:
            _journal(directory, event, 'continuation_limit', fingerprint=fingerprint, count=count, incomplete=True)
            return _hook_output(name, context='The same unresolved gate condition reached the continuation limit. Stop repeated scheduling and report incomplete; independent delivery verification still rejects it.')
        state['continuations'][fingerprint] = count + 1
        _journal(directory, event, 'continue_verification', fingerprint=fingerprint, count=count + 1, incomplete=True)
        return _hook_output(name, deny='Luna verification is incomplete for the current modifications. Run the shared project verifier and fix applicable failures. Do not close the work item or fabricate manual approvals.')


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path)
    args = parser.parse_args(argv)
    event: dict = {}
    try:
        raw = sys.stdin.buffer.read(MAX_INPUT + 1)
        if len(raw) > MAX_INPUT:
            raise ValueError('hook.input_too_large')
        event = json.loads(raw)
        if not isinstance(event, dict):
            raise ValueError('hook.input_not_object')
        root = args.root or find_root(event.get('cwd', os.getcwd()))
        result = handle_event(root, event)
        print(json.dumps(result, ensure_ascii=False))
        return 0
    except Exception as error:
        # Code 2 is the documented rejection/feedback path, unlike a generic
        # crash.  It cannot retroactively undo a PostToolUse side effect.
        reason = 'hook.adapter_error: ' + type(error).__name__ + ': ' + str(error)[:1000]
        print(reason, file=sys.stderr)
        if isinstance(event, dict) and event.get('hook_event_name') == 'Stop':
            # An unreadable state must not turn Stop's exit-2 continuation into
            # an endless loop. No verified_stop or gate receipt is issued.
            print(json.dumps({'systemMessage': reason + '; stop repeated scheduling and report incomplete. Independent delivery verification is still required.'}))
            return 0
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
