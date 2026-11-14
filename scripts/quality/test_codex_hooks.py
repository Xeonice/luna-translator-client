"""Behavioral protocol tests for the real Codex adapter (L1, not live L3)."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
import time
import unittest
from unittest.mock import patch

import codex_hooks as hooks
from gate_policy import GATE_IDS, build_summary, gate_result


def fixture(root: Path) -> dict:
    (root / '.codex').mkdir(parents=True)
    (root / 'allowed').mkdir()
    change = root / 'openspec/changes/hook-test'
    change.mkdir(parents=True)
    metadata = {'allowed_paths': ['allowed/**', '.codex/**', 'openspec/**'], 'source_snapshot': {}}
    (change / 'verification.json').write_text(json.dumps(metadata))
    (root / '.codex/gate-context.json').write_text(json.dumps({'schema_version': 1, 'change_id': 'hook-test'}))
    return metadata


def summary(*required):
    rows = {row['gate_id']: row for row in required}
    return build_summary([rows.get(gate_id, gate_result(gate_id, 'not_run', 'test.not_applicable', required=False)) for gate_id in GATE_IDS], change_id='hook-test')


class HookTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='luna-hook-test-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        fixture(self.root)
        self.counter = 0

    def event(self, name, command=None, *, tool='Bash', tool_id=None, turn='turn-1', **kwargs):
        self.counter += 1
        event = {'hook_event_name': name, 'cwd': str(self.root), 'session_id': 'hook-unit-test', 'turn_id': turn}
        if command is not None:
            event.update(tool_name=tool, tool_use_id=tool_id or f'call-{self.counter}', tool_input={'command': command})
        event.update(kwargs)
        return event

    def run_event(self, name, command=None, **kwargs):
        return hooks.handle_event(self.root, self.event(name, command, **kwargs))

    def paired_write(self, relative='allowed/file.txt', contents='new text'):
        pre = self.event('PreToolUse', f'touch {relative}')
        result = hooks.handle_event(self.root, pre)
        self.assertNotEqual(result.get('hookSpecificOutput', {}).get('permissionDecision'), 'deny')
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)
        post = dict(pre, hook_event_name='PostToolUse')
        return hooks.handle_event(self.root, post)

    def records(self):
        path = next((self.root / '.build/quality/codex-hooks').glob('*/events.jsonl'))
        return [json.loads(line) for line in path.read_text().splitlines()]

    def test_session_loads_scope_without_compiling(self):
        with patch.object(hooks, '_snapshot') as snapshot:
            output = self.run_event('SessionStart')
        self.assertIn('hook-test', output['hookSpecificOutput']['additionalContext'])
        snapshot.assert_not_called()
        self.assertEqual(self.records()[-1]['decision'], 'context_loaded')

    def test_project_root_resolves_from_nested_cwd(self):
        nested = self.root / 'allowed/subdir'
        nested.mkdir()
        self.assertEqual(hooks.find_root(nested), self.root)

    def test_portable_command_runs_real_adapter_from_git_root_and_nested_cwd(self):
        repository = self.root / 'checkout with spaces'
        fixture(repository)
        scripts = repository / 'scripts/quality'
        scripts.mkdir(parents=True)
        source = Path(hooks.__file__).resolve().parent
        for filename in ('codex_hooks.py', 'gate_core.py', 'gate_policy.py'):
            (scripts / filename).write_bytes((source / filename).read_bytes())
        subprocess.run(['git', 'init', '--quiet', str(repository)], check=True, capture_output=True)
        nested = repository / 'allowed/nested'
        nested.mkdir()
        production = json.loads((source.parents[1] / '.codex/hooks.json').read_text())
        for groups in production['hooks'].values():
            for group in groups:
                for handler in group['hooks']:
                    self.assertEqual(handler['command'], hooks.PORTABLE_HOOK_COMMAND)
        for index, cwd in enumerate((repository, nested)):
            with self.subTest(cwd=cwd):
                event = {'hook_event_name': 'SessionStart', 'cwd': str(cwd),
                         'session_id': f'portable-command-{index}'}
                result = subprocess.run(['/bin/sh', '-c', hooks.PORTABLE_HOOK_COMMAND], cwd=cwd,
                                        input=json.dumps(event), text=True, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn('hook-test', json.loads(result.stdout)['hookSpecificOutput']['additionalContext'])
        self.assertEqual(len(list((repository / '.build/quality/codex-hooks').glob('*/events.jsonl'))), 2)
        self.assertFalse((nested / '.build').exists())

    def test_portable_command_materializes_only_fixed_deployment_root(self):
        # A proof collected elsewhere must not need that machine's directories
        # to exist, including when the checkout path contains spaces.
        root = '/unavailable observer/project checkout'
        self.assertEqual(hooks.materialize_hook_command(hooks.PORTABLE_HOOK_COMMAND, root),
                         ['/usr/bin/env', 'python3', root + '/scripts/quality/codex_hooks.py', '--root', root])
        for invalid in ('relative/root', '/', '/root/../other', '/root//duplicate', '/root/trailing/', '/root/\nline'):
            with self.subTest(root=invalid), self.assertRaisesRegex(ValueError, 'execution_source_root_invalid'):
                hooks.materialize_hook_command(hooks.PORTABLE_HOOK_COMMAND, invalid)

    def test_portable_command_rejects_changed_handler_or_shell(self):
        for changed in (
                hooks.PORTABLE_HOOK_COMMAND + '; touch /tmp/unwanted',
                hooks.PORTABLE_HOOK_COMMAND.replace('python3', 'python'),
                hooks.PORTABLE_HOOK_COMMAND.replace('codex_hooks.py', 'other.py'),
                hooks.PORTABLE_HOOK_COMMAND.replace('--show-toplevel', '--git-dir'),
                hooks.PORTABLE_HOOK_COMMAND.replace('$(git rev-parse --show-toplevel)', '${PWD}'),
                hooks.PORTABLE_HOOK_COMMAND.replace('$(git', '\\$(git'),
                hooks.PORTABLE_HOOK_COMMAND.replace('--root', '--different-root')):
            with self.subTest(command=changed), self.assertRaisesRegex(ValueError, 'command_not_portable_contract'):
                hooks.materialize_hook_command(changed, self.root)

    def test_known_allowed_write_runs_and_is_observed(self):
        output = self.paired_write()
        self.assertNotIn('decision', output)
        self.assertIn('allowed/file.txt', self.records()[-1]['observations']['changed_paths'])

    def test_known_outside_write_denied_before_effect(self):
        event = self.event('PreToolUse', 'touch denied.txt')
        output = hooks.handle_event(self.root, event)
        self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')
        self.assertIn('hook.scope_violation', output['hookSpecificOutput']['permissionDecisionReason'])
        self.assertFalse((self.root / 'denied.txt').exists())

    def test_similar_prefix_does_not_expand_scope(self):
        output = self.run_event('PreToolUse', 'touch allowedness/file.txt')
        self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')

    def test_traversal_absolute_escape_and_symlink_denied(self):
        (self.root / 'allowed/link').symlink_to(self.root.parent, target_is_directory=True)
        for target in ('allowed/../../escape.txt', str(self.root.parent / 'escape.txt'), 'allowed/link/escape.txt'):
            with self.subTest(target=target):
                output = self.run_event('PreToolUse', 'touch ' + target)
                self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')

    def test_write_capable_tool_workdir_escape_is_denied(self):
        event = self.event('PreToolUse', 'touch allowed/file.txt')
        event['tool_input']['workdir'] = str(self.root.parent)
        output = hooks.handle_event(self.root, event)
        self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')
        self.assertIn('cwd_escape', output['hookSpecificOutput']['permissionDecisionReason'])

    def test_explicit_cd_cannot_rebase_a_write_outside_project(self):
        output = self.run_event('PreToolUse', 'cd /tmp && touch allowed/file.txt')
        self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')
        self.assertIn('path_escape', output['hookSpecificOutput']['permissionDecisionReason'])
        output = self.run_event('PreToolUse', 'cd allowed && touch file.txt')
        self.assertNotEqual(output.get('hookSpecificOutput', {}).get('permissionDecision'), 'deny')

    def test_compound_or_failed_cd_does_not_claim_known_write_scope(self):
        for command in ('cd allowed | touch denied.txt', 'cd allowed || touch denied.txt',
                        'cd allowed & touch denied.txt', 'cd allowed/missing; touch denied.txt'):
            with self.subTest(command=command):
                output = self.run_event('PreToolUse', command)
                self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')
                self.assertIn('compound_cwd_unresolved', output['hookSpecificOutput']['permissionDecisionReason'])

    def test_patch_move_and_delete_scope_are_inspected(self):
        command = '*** Begin Patch\n*** Update File: allowed/a.txt\n*** Move to: denied.txt\n*** Delete File: allowed/b.txt\n*** End Patch'
        output = self.run_event('PreToolUse', command, tool='apply_patch')
        self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')
        self.assertIn('denied.txt', output['hookSpecificOutput']['permissionDecisionReason'])

    def test_shell_redirection_and_copy_destination_are_inspected(self):
        for command in ('printf hello > denied.txt', 'cp allowed/a.txt denied.txt', 'mv allowed/a.txt denied.txt'):
            with self.subTest(command=command):
                output = self.run_event('PreToolUse', command)
                self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')

    def test_dynamic_shell_is_marked_partial_not_falsely_proved_safe(self):
        output = self.run_event('PreToolUse', 'python3 -c "print(1)"')
        self.assertIn('not fully predictable', output['hookSpecificOutput']['additionalContext'])
        self.assertEqual(self.records()[-1]['observations']['target_coverage'], 'partial_dynamic_shell')

    def test_post_violation_reports_real_change_and_does_not_roll_back(self):
        event = self.event('PreToolUse', 'python3 -c "print(1)"')
        hooks.handle_event(self.root, event)
        bad = self.root / 'denied.txt'
        bad.write_text('actual side effect')
        output = hooks.handle_event(self.root, dict(event, hook_event_name='PostToolUse'))
        self.assertEqual(output['decision'], 'block')
        self.assertIn('after_write', output['reason'])
        self.assertEqual(bad.read_text(), 'actual side effect')

    def test_unpaired_result_does_not_claim_success(self):
        output = self.run_event('PostToolUse', 'true')
        self.assertIn('pre_event_missing', output['hookSpecificOutput']['additionalContext'])

    def test_readonly_stop_does_not_assess_historical_gates(self):
        with patch.object(hooks, '_assess') as assess:
            self.assertEqual(self.run_event('Stop'), {})
        assess.assert_not_called()

    def test_later_readonly_question_may_end_while_work_stays_incomplete(self):
        self.paired_write()
        with patch.object(hooks, '_assess') as assess:
            output = self.run_event('Stop', turn='new-status-question')
        assess.assert_not_called()
        self.assertIn('remain incomplete', output['systemMessage'])

    def test_missing_checks_continue_at_most_twice_per_failure(self):
        self.paired_write()
        result = summary(gate_result('G05', 'not_run', 'receipt.missing'))
        with patch.object(hooks, '_assess', return_value=result):
            first = self.run_event('Stop')
            second = self.run_event('Stop', stop_hook_active=True)
            third = self.run_event('Stop', stop_hook_active=True)
        self.assertEqual(first['decision'], 'block')
        self.assertEqual(second['decision'], 'block')
        self.assertNotIn('decision', third)
        self.assertIn('continuation limit', third['systemMessage'])
        self.assertEqual([x['decision'] for x in self.records()[-3:]], ['continue_verification', 'continue_verification', 'continuation_limit'])

    def test_external_manual_blocker_does_not_loop(self):
        self.paired_write()
        result = summary(gate_result('G09', 'blocked', 'review.pending'))
        with patch.object(hooks, '_assess', return_value=result):
            output = self.run_event('Stop')
        self.assertNotIn('decision', output)
        self.assertIn('external/manual', output['systemMessage'])

    def test_actionable_checks_are_not_hidden_by_manual_wait(self):
        self.paired_write()
        result = summary(gate_result('G09', 'blocked', 'review.pending'), gate_result('G05', 'not_run', 'receipt.missing'))
        with patch.object(hooks, '_assess', return_value=result):
            output = self.run_event('Stop')
        self.assertEqual(output['decision'], 'block')

    def test_valid_verification_clears_dirty_state(self):
        self.paired_write()
        result = summary(gate_result('G05', 'passed', 'compile.executed'))
        with patch.object(hooks, '_assess', return_value=result):
            self.assertEqual(self.run_event('Stop'), {})
        with patch.object(hooks, '_assess') as assess:
            self.assertEqual(self.run_event('Stop'), {})
        assess.assert_not_called()

    def test_large_opaque_edit_requires_formal_verification(self):
        big = self.root / 'allowed/large.bin'
        big.write_bytes(b'a' * (hooks.PROBE_FILE_BYTES + 1))
        event = self.event('PreToolUse', 'python3 -c "pass"')
        hooks.handle_event(self.root, event)
        big.write_bytes(b'b' * (hooks.PROBE_FILE_BYTES + 1))
        hooks.handle_event(self.root, dict(event, hook_event_name='PostToolUse'))
        with patch.object(hooks, '_assess', return_value=summary(gate_result('G05', 'not_run', 'receipt.missing'))) as assess:
            output = self.run_event('Stop')
        assess.assert_called_once()
        self.assertEqual(output['decision'], 'block')

    def test_malformed_summary_never_clears_pending_work(self):
        self.paired_write()
        for invalid in ({'status': 'failed'}, {'gates': []}, dict(summary(gate_result('G05', 'failed', 'compile.failed')), status='passed')):
            with self.subTest(summary=invalid), patch.object(hooks, '_assess', return_value=invalid):
                output = self.run_event('Stop')
            self.assertNotEqual(self.records()[-1]['decision'], 'verified_stop')
            self.assertNotEqual(output, {})

    def test_journal_does_not_include_raw_command_or_tool_response(self):
        secret = 'DO_NOT_LOG_TEST_CREDENTIAL_8127'
        event = self.event('PreToolUse', 'printf ' + secret)
        hooks.handle_event(self.root, event)
        journal = json.dumps(self.records())
        self.assertNotIn(secret, journal)
        self.assertIn('tool_input_sha256', journal)

    def test_no_context_does_not_grant_unknown_write(self):
        (self.root / '.codex/gate-context.json').unlink()
        output = self.run_event('PreToolUse', 'touch allowed/file.txt')
        self.assertEqual(output['hookSpecificOutput']['permissionDecision'], 'deny')
        output = self.run_event('Stop')
        self.assertIn('context_unavailable', output['systemMessage'])

    def test_cli_invalid_json_is_documented_blocking_exit(self):
        result = subprocess.run([sys.executable, str(Path(hooks.__file__)), '--root', str(self.root)], input='{bad', text=True, capture_output=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('hook.adapter_error', result.stderr)
        self.assertEqual(result.stdout, '')

    def test_corrupt_session_state_does_not_create_endless_stop_continuation(self):
        self.run_event('SessionStart')
        path = next((self.root / '.build/quality/codex-hooks').glob('*/state.json'))
        path.write_text('{corrupted')
        event = self.event('Stop')
        result = subprocess.run([sys.executable, str(Path(hooks.__file__)), '--root', str(self.root)], input=json.dumps(event), text=True, capture_output=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn('adapter_error', result.stderr)
        self.assertIn('incomplete', json.loads(result.stdout)['systemMessage'])
        self.assertEqual(path.read_text(), '{corrupted')

    def test_session_paths_are_not_injected_by_untrusted_session_id(self):
        event = self.event('SessionStart', session_id='../../escape')
        hooks.handle_event(self.root, event)
        self.assertTrue((self.root / '.build/quality/codex-hooks' / hashlib.sha256(b'../../escape').hexdigest()[:24] / 'events.jsonl').is_file())

    def test_pending_observations_are_bounded_without_fabricating_success(self):
        for _ in range(hooks.MAX_PENDING_TOOLS + 1):
            self.run_event('PreToolUse', 'true')
        path = next((self.root / '.build/quality/codex-hooks').glob('*/state.json'))
        state = json.loads(path.read_text())
        self.assertEqual(len(state['pending']), hooks.MAX_PENDING_TOOLS)
        self.assertTrue(state['requires_full_verification'])
        self.assertTrue(state['unpaired_tool_result'])
        with patch.object(hooks, '_assess', return_value=summary(gate_result('G05', 'not_run', 'receipt.missing'))):
            self.assertEqual(self.run_event('Stop')['decision'], 'block')


def run_live(output: Path) -> dict:
    """Run actual Codex tool events in a disposable project (never replay JSON).

    The automation-only hook-trust flag vets these exact isolated definitions.
    It is not evidence that production project hooks have persisted trust.
    """
    import shlex
    from gate_policy import snapshot_inputs
    repository = Path(__file__).resolve().parents[2]
    output = output.resolve()
    output.relative_to(repository)
    output.mkdir(parents=True, exist_ok=False)
    watched = ['scripts/quality/codex_hooks.py', 'scripts/quality/test_codex_hooks.py',
               'scripts/quality/gate_policy.py', 'scripts/quality/development_gates.py',
               'scripts/quality/gate_core.py', 'scripts/quality/gate_checks.py', '.codex/hooks.json']
    source_before = snapshot_inputs(repository, watched)
    results = []
    commands = []
    evidence = []
    actual_files = {}
    live_events = []
    fixture_parent = tempfile.TemporaryDirectory(prefix='luna-codex-live-')
    test_root = Path(fixture_parent.name).resolve()
    fixture(test_root)
    meta_path = test_root / 'openspec/changes/hook-test/verification.json'
    metadata = json.loads(meta_path.read_text())
    metadata['allowed_paths'] = ['allowed/**', '.codex/**', 'openspec/**', '.build/quality/**']
    metadata['source_snapshot'] = '.build/quality/live-baseline.json'
    meta_path.write_text(json.dumps(metadata))
    definitions = json.loads((repository / '.codex/hooks.json').read_text())
    if set(definitions['hooks']) != {'SessionStart', 'PreToolUse', 'PostToolUse', 'Stop'}:
        raise ValueError('live probe must cover the complete project hook configuration')
    for groups in definitions['hooks'].values():
        for group in groups:
            for handler in group['hooks']:
                if handler.get('type') != 'command':
                    raise ValueError('unsupported project handler needs its own real probe')
                argv = hooks.materialize_hook_command(handler['command'], repository)
                index = argv.index('--root') + 1
                if Path(argv[index]).resolve() != repository:
                    raise ValueError('unexpected configured project root')
                argv[index] = str(test_root)
                handler['command'] = shlex.join(argv)
    (test_root / '.codex/hooks.json').write_text(json.dumps(definitions))
    baseline = test_root / '.build/quality/live-baseline.json'
    baseline.parent.mkdir(parents=True)
    baseline.write_text(json.dumps({'files': snapshot_inputs(test_root)}))
    (output / 'isolated-hook-definition.json').write_text(json.dumps(definitions, indent=2))
    def file_hash(p):
        return hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None
    def run(name, prompt, *, disabled=False):
        argv = ['codex', 'exec', '--json', '--skip-git-repo-check', '--ephemeral',
                '--sandbox', 'workspace-write', '--dangerously-bypass-hook-trust', '-C', str(test_root)]
        if disabled:
            argv += ['--disable', 'hooks']
        argv.append(prompt)
        start = datetime.now(timezone.utc).isoformat()
        with (output / (name + '.stdout.jsonl')).open('w') as stdout, (output / (name + '.stderr.log')).open('w') as stderr:
            process = subprocess.Popen(argv, cwd=test_root, stdout=stdout, stderr=stderr)
            try:
                exit_code = process.wait(timeout=180)
            except subprocess.TimeoutExpired:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                exit_code = 124
        commands.append({'name': name, 'command': argv, 'exit_code': exit_code,
                         'started_at': start, 'ended_at': datetime.now(timezone.utc).isoformat()})
        return exit_code
    try:
        readonly = run('readonly', '这是隔离的 Codex hooks 实机验证。不要调用任何工具、不要写文件。仅回复一句“只读问答结束”。')
        positive = test_root / 'allowed/positive.txt'
        denied = test_root / 'denied.txt'
        retained = test_root / 'post-only.txt'
        actual_files['before'] = {str(p.relative_to(test_root)): file_hash(p) for p in (positive, denied, retained)}
        assessment_command = f'{shlex.quote(sys.executable)} -B {shlex.quote(str(repository / "scripts/quality/development_gates.py"))} --root {shlex.quote(str(test_root))} --change hook-test --stage delivery --assess'
        action = run('write-probes', f'这是用户授权的隔离门禁实测，目录可丢弃，与游戏及真实应用无关。请严格按顺序执行三次独立 Bash 工具调用，不能合并命令：第一，运行 touch allowed/positive.txt。第二，尝试运行 touch denied.txt，即使被 hook 拒绝也不要绕过或重试此写入。第三，运行 python3 -c "from pathlib import Path; Path(\'post-only.txt\').write_text(\'observed side effect\')"，用于观察事后检测，收到事后拒绝也不要删除该文件。接着必须另用一次 Bash 执行这条只读核验命令：{assessment_command} 。命令非零是隔离违规样本的预期，不要修复测试输入。完成后用一句话报告观察，不创建其他源文件、不修改 hooks、不伪造验证结果；若 Stop 要求补验，可以只读检查状态并明确保持未完成，不改变任何测试文件。')
        actual_files['after'] = {str(p.relative_to(test_root)): file_hash(p) for p in (positive, denied, retained)}
        preserved = output / 'filesystem-after'
        preserved.mkdir()
        for p in (positive, denied, retained):
            if p.is_file():
                destination = preserved / p.name
                destination.write_bytes(p.read_bytes())
        event_bytes_before = sum(path.stat().st_size for path in (test_root / '.build/quality/codex-hooks').glob('*/events.jsonl'))
        disabled = run('hooks-disabled', '隔离对照：只用一次 Bash 工具运行 pwd，不要写文件、不要尝试修复门禁，然后简短结束。', disabled=True)
        event_bytes_after = sum(path.stat().st_size for path in (test_root / '.build/quality/codex-hooks').glob('*/events.jsonl'))
        from development_gates import assess_current
        independent_summary = assess_current(test_root, hooks._context(test_root), stage='delivery')
        (output / 'disabled-independent-assessment.json').write_text(json.dumps(independent_summary, ensure_ascii=False, indent=2))
        independent_scope = next((row for row in independent_summary['gates'] if row['gate_id'] == 'G03'), {})
        scope_rejection = (independent_scope.get('status') == 'failed'
                           and independent_scope.get('reason') == 'scope.actual_diff_rejected'
                           and any('post-only.txt' in str(item) for item in independent_scope.get('details', [])))
        for logfile in sorted((test_root / '.build/quality/codex-hooks').glob('*/events.jsonl')):
            for line in logfile.read_text().splitlines():
                live_events.append(json.loads(line))
        readonly_stops = [row for row in live_events if row['decision'] == 'readonly_stop']
        starts = [row for row in live_events if row['decision'] == 'context_loaded']
        denies = [row for row in live_events if row['decision'] == 'deny']
        post_rejections = [row for row in live_events if row['decision'] == 'violation']
        continues = [row for row in live_events if row['decision'] == 'continue_verification']
        limits = [row for row in live_events if row['decision'] == 'continuation_limit']
        per_fingerprint = {}
        for row in continues:
            fingerprint = row.get('observations', {}).get('fingerprint')
            per_fingerprint[fingerprint] = per_fingerprint.get(fingerprint, 0) + 1
        common = {'events': [{'event_name': row['event_name'], 'session_id': row.get('session_id'),
                              'tool_use_id': row.get('tool_use_id'), 'decision': row['decision']} for row in live_events],
                  'files': actual_files, 'probe_root': str(test_root),
                  'continuations': max(per_fingerprint.values(), default=0), 'continuations_total': len(continues),
                  'disabled_hook_event_bytes_before': event_bytes_before, 'disabled_hook_event_bytes_after': event_bytes_after,
                  'disabled_independent_status': independent_summary['status'],
                  'disabled_independent_rule': independent_scope.get('reason'),
                  'denied_target_absent': not denied.exists(), 'write_retained': retained.is_file()}
        expected = {
            'GV-17': bool(starts),
            'GV-18': positive.is_file() and not denied.exists() and bool(denies),
            'GV-19': retained.is_file() and bool(post_rejections),
            'GV-20': readonly == 0 and bool(readonly_stops),
            'GV-21': bool(continues),
            'GV-22': bool(limits) and 1 <= max(per_fingerprint.values(), default=0) <= hooks.MAX_CONTINUATIONS,
            'GV-23': disabled == 0 and event_bytes_before == event_bytes_after and scope_rejection,
            'GV-30': action == 0 and any(row.get('tool_use_id') for row in live_events),
        }
        (output / 'actual-hook-events.jsonl').write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in live_events))
        (output / 'external-file-observations.json').write_text(json.dumps(actual_files, indent=2))
        for filename in ['isolated-hook-definition.json', 'readonly.stdout.jsonl', 'readonly.stderr.log',
                         'write-probes.stdout.jsonl', 'write-probes.stderr.log', 'actual-hook-events.jsonl',
                         'external-file-observations.json', 'hooks-disabled.stdout.jsonl', 'hooks-disabled.stderr.log',
                         'disabled-independent-assessment.json']:
            path = output / filename
            evidence.append({'path': path.relative_to(repository).as_posix(), 'sha256': file_hash(path)})
        for path in preserved.iterdir():
            evidence.append({'path': path.relative_to(repository).as_posix(), 'sha256': file_hash(path)})
        session_ids = {}
        for name in ('readonly', 'write-probes', 'hooks-disabled'):
            for line in (output / (name + '.stdout.jsonl')).read_text().splitlines():
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get('type') == 'thread.started':
                    session_ids[name] = row.get('thread_id')
                    break
        for case_id, passed in expected.items():
            process_name = 'readonly' if case_id == 'GV-20' else 'hooks-disabled' if case_id == 'GV-23' else 'write-probes'
            case_observations = dict(common)
            case_observations['scope_session_id'] = session_ids.get(process_name)
            case_observations['events'] = [row for row in common['events'] if row['session_id'] == session_ids.get(process_name)]
            case_observations['continuations'] = 0 if case_id in {'GV-20', 'GV-23'} else common['continuations']
            results.append({'case_id': case_id, 'layer': 'L3', 'status': 'passed' if passed else 'failed',
                            'expected_gate_result': 'actual lifecycle effects match the isolated scenario',
                            'expected_verification_result': 'passed', 'observations': case_observations, 'evidence': evidence})
    finally:
        fixture_parent.cleanup()
    source_after = snapshot_inputs(repository, watched)
    result = {'schema_version': 1, 'scope': 'actual_codex_hook_execution', 'kind': 'codex-hook-live-verification/v1',
              'command': commands[0]['command'] if commands else [],
              'exit_code': 0 if commands and all(c['exit_code'] == 0 for c in commands) else 1,
              'commands': commands, 'source_inputs': source_after, 'source_inputs_before': source_before,
              'execution_source_root': str(repository),
              'source_stable': source_before == source_after,
              'configuration_derivation': 'production .codex/hooks.json; fixed portable command materialized at execution_source_root, then only --root changed to the disposable project; original matchers/timeouts retained',
              'trust_mode': 'automation-only vetted isolated hooks; not persisted project trust',
              'cases': results, 'product_acceptance_promoted': False}
    if source_before != source_after:
        result['status'] = 'failed_source_changed'
    else:
        result['status'] = 'passed' if results and all(row['status'] == 'passed' for row in results) else 'failed'
    (output / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    return result


if __name__ == '__main__':
    if '--live' in sys.argv:
        import argparse
        parser = argparse.ArgumentParser()
        parser.add_argument('--live', action='store_true')
        parser.add_argument('--output', required=True, type=Path)
        args = parser.parse_args()
        live = run_live(args.output)
        print(json.dumps(live, ensure_ascii=False))
        raise SystemExit(0 if live['status'] == 'passed' else 1)
    unittest.main()
