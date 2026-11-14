"""Finite current-source React execution; never Native or whole-product approval."""
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import subprocess

from gate_core import digest, safe_path
from current_reference_catalogue import load

CHECK_ID = 'product-current-design-react-tests'
RUNNER = 'scripts/report/verify_current_design_react.mjs'
COMMAND = ('node', RUNNER)
STATE_IDS = frozenset({
    'all-page-main-session-normal',
    'all-page-settings-general-normal',
    'all-page-subpage-readingpanel-dataset-body-retired-surface-absent',
    'all-page-subpage-modelconfigscreen-tab-connections-retired-surface-absent',
})


def _ref(root, path):
    return {'path': path.relative_to(root).as_posix(), 'sha256': digest(path)}


def execute(root, context):
    """Own a fresh command, retain failures, then invoke both raw validators."""
    from package_identity import inspect_package
    from current_design_react_inspect import inspect
    from current_design_react_recompute import recompute
    root = Path(root)
    if (context['adapter_id'] != CHECK_ID or context['command'] != list(COMMAND)
            or context['layer'] != 'real_react' or context['state_ids'] != sorted(STATE_IDS)):
        raise ValueError('finite code-owned current React invocation differs')
    output = safe_path(root, context['output']); request = output / 'request.json'
    if load(request) != context: raise ValueError('actual dispatcher request differs')
    package = safe_path(root, context['artifact_path'])
    # The controlled component layer still binds the same real final package.
    # A nonempty text file, an old .app or a JSON assertion cannot substitute.
    package_result = inspect_package(root, package, context['source_tree_sha256'], output / 'package-inspection')
    node = shutil.which('node')
    if node is None: raise ValueError('node executable is unavailable')
    command = [str(Path(node).resolve(strict=True)), str(safe_path(root, RUNNER)),
               '--request', str(request), '--output', str(output)]
    started = datetime.now(timezone.utc).isoformat()
    execution = output / 'actual-command.json'
    try:
        completed = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=600)
    except (subprocess.SubprocessError, OSError) as error:
        def captured(value): return value.decode('utf-8', errors='replace') if isinstance(value, bytes) else value or ''
        execution.write_text(json.dumps({'command': command, 'cwd': str(root), 'started_at': started,
            'finished_at': datetime.now(timezone.utc).isoformat(), 'exit_code': None,
            'timed_out': isinstance(error, subprocess.TimeoutExpired), 'error': str(error),
            'stdout': captured(getattr(error, 'stdout', None)), 'stderr': captured(getattr(error, 'stderr', None)),
            'package_inspection': package_result}, ensure_ascii=False, indent=2) + '\n')
        raise ValueError('actual current React command timed out or could not run; raw command failure retained') from error
    finished = datetime.now(timezone.utc).isoformat()
    execution.write_text(json.dumps({'command': command, 'cwd': str(root), 'started_at': started,
        'finished_at': finished, 'exit_code': completed.returncode, 'stdout': completed.stdout,
        'stderr': completed.stderr, 'package_inspection': package_result}, ensure_ascii=False, indent=2) + '\n')
    report_path = output / 'capture/verification.json'; report = load(report_path)
    passed = completed.returncode == 0 and report.get('status') == 'passed'
    receipt = {key: context[key] for key in ('schema_version', 'adapter_id', 'target_id', 'composition_id',
        'layer', 'design_closure_sha256', 'source_tree_sha256', 'artifact_sha256', 'command', 'execution_nonce', 'state_ids')}
    receipt.update(status='passed' if passed else 'failed', errors=[] if passed else ['actual finite current React execution failed'],
        producer_kind='real_react', prototype_substitution=False, native_verified=False,
        started_at=report['started_at'], finished_at=report['finished_at'],
        assertion_count=sum(row['assertion_count'] for row in report['results']),
        skipped_assertions=sum(row['skipped_assertions'] for row in report['results']),
        report=_ref(root, report_path), execution=_ref(root, execution))
    inspect_context = {key: receipt[key] for key in ('target_id', 'composition_id', 'design_closure_sha256',
        'layer', 'state_ids', 'artifact_sha256', 'execution_nonce')}
    findings = {'inspection': inspect(root, receipt, inspect_context),
                'independent_recomputation': recompute(root, receipt, inspect_context)}
    (output / 'raw-validation.json').write_text(json.dumps(findings, ensure_ascii=False, indent=2) + '\n')
    # Keep the failed raw receipt too; only the dispatcher may create its accepted
    # receipt.json/invocation.json after successful independent validation.
    if findings['inspection'] or findings['independent_recomputation']:
        receipt['status'] = 'failed'
        receipt['errors'] = findings['inspection'] + findings['independent_recomputation']
    (output / 'producer-receipt.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    if load(request) != context: raise ValueError('runner rewrote dispatcher request')
    return receipt
