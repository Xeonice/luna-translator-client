"""Execute code-registered production checks and bind fresh, inspected artifacts to source.

Browser tests use real product React components and an explicitly controlled test port.
They are deliberately distinct from packaged/native/game acceptance.
"""
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import subprocess
import time
import uuid

from gate_core import digest, safe_path, source_files
from design_source_versions import load_source_version

PRODUCT_CAPTURE_CHECKS = {
    'product-ui-capture': ('node', 'apps/desktop/tests/visual/verify-ui.mjs',
                           '--reference-catalogue', 'docs/design/first-product-v1/acceptance-v3/catalogue.json'),
    'product-ui-capture-v4': ('node', 'apps/desktop/tests/visual/verify-ui.mjs',
                            '--reference-catalogue', 'docs/design/first-product-v1/acceptance-v4/catalogue.json'),
}
PRODUCT_TEST_CHECKS = {
    'product-preview-visual-tests': ('node', 'scripts/report/verify_preview_visuals.mjs'),
    'product-cp01-reading-tests': ('node', 'scripts/report/verify_cp01_reading.mjs'),
    'product-settings-controls-tests': ('node', 'scripts/report/verify_settings_controls.mjs'),
    'product-core-tests': ('cargo', 'test', '--locked', '--offline', '-p', 'app-core', '--lib', '--', '--nocapture'),
    'product-infra-tests': ('cargo', 'test', '--locked', '--offline', '-p', 'app-infra', '--lib', '--tests', '--', '--nocapture'),
    'product-native-ipc-tests': ('cargo', 'test', '--locked', '--offline', '-p', 'luna-reader', '--bin', 'luna-reader', '--', '--nocapture'),
    'product-hook-client-tests': ('cargo', 'test', '--locked', '--offline', '-p', 'hook-client', '--lib', '--', '--nocapture'),
    'product-target-probe-tests': ('cargo', 'test', '--locked', '--offline', '-p', 'lunahook-host', '--example', 'target_probe', '--', '--nocapture'),
    'product-window-title-proof-api-tests': ('python3', 'scripts/quality/window_title_proof_api.py'),
}
WINDOW_TITLE_PROOF_CHECK = 'product-window-title-proof-api-tests'
WINDOW_TITLE_PROOF_SCOPE = 'actual_current_Rust_target_probe_against_owned_nonactivating_WinAPI_fixtures_only'
WINDOW_TITLE_PROOF_SOURCES = frozenset({
    'crates/hook-client/src/window_binding.rs', 'crates/lunahook-host/examples/target_probe.rs',
    'scripts/quality/window_title_proof_api.py', 'scripts/quality/fixtures/window_title_fixture.c',
    'Cargo.lock', 'Cargo.toml', 'crates/lunahook-host/Cargo.toml',
})
WINDOW_TITLE_PROOF_TARGETS = {'x86': 'i686-pc-windows-gnullvm', 'x64': 'x86_64-pc-windows-gnullvm'}
REQUIRED_WINDOW_TITLE_PROOF_CASES = frozenset(
    f'WTPA-{fixture}-{probe}-{case}'
    for fixture in ('x86', 'x64') for probe in ('x86', 'x64')
    for case in ('ansi-prewrite-rejected', 'unicode-exact-restore')
) | frozenset(
    f'WTPA-x86-{probe}-{case}' for probe in ('x86', 'x64') for case in (
        'observer-eof-restores', 'observer-thirdparty-preserved',
        'identity-pid-rejected', 'identity-path-rejected', 'identity-creation-rejected',
        'caption-truncated-prewrite-rejected', 'caption-invalid-utf16-prewrite-rejected',
        'handler-rejected-explicit-restore', 'stdout-failed-explicit-restore',
        'restore-invalid-utf16-not-acknowledged', 'timeout-late-write-not-falsely-restored',
    )
)
# One actual native/package runner. Registration itself never grants coverage.
RUNTIME_STABILITY_CHECK = 'product-packaged-stability-tests'
PRODUCT_RUNTIME_CHECKS = {RUNTIME_STABILITY_CHECK: ('python3', 'scripts/quality/runtime_stability.py')}
REFERENCE_CATALOGUE = 'docs/design/first-product-v1/acceptance-v3/catalogue.json'
REQUIRED_CAPTURES = {
    'empty-library': '38-library-empty.png',
    'add-game': '40-add-game-review.png',
    'target-review': '42-window-mapping.png',
    'settings-connections': '31-dark-settings.png',
    'reading-hud': '09-hud-detail.png',
    'learning-fixed': '11-learning-unconfigured.png',
    'library-batch': '46-custom-batch-review.png',
    'display-preferences': '32-large-opaque-hud.png',
    'model-scheduling': '22-model-entities.png',
    'connection-proxy': '23-connection-entities.png',
    'history-earlier': '20-history.png',
    'wordcard-native-size': '10-local-word-card.png',
    'glossary-editing': '22-model-entities.png',
    'dictionary-distinct-headwords': '10-local-word-card.png',
    'learning-japanese-linebreak': '11-learning-unconfigured.png',
}
REFERENCE_CATALOGUE_V4 = 'docs/design/first-product-v1/acceptance-v4/catalogue.json'
BLOCKED_REFERENCE_STATUS = 'blocked_pending_replacement_reference'
HISTORICAL_CAPTURE_BLOCKS = {
    'session-current-body': 'retired_session_tabs',
    'session-current-extraction': 'retired_session_tabs',
    'session-current-hook-events': 'retired_session_tabs',
    'preview-off': 'retired_manual_preview_off',
    'target-review-preview-off': 'retired_manual_preview_off',
}
# The independent adapters are test-only source, outside the design directory.
# Each exact input also needs its own independently reviewed authority pin.
V4_REFERENCE_PRODUCER_INPUTS = frozenset({
    'apps/desktop/tests/visual/' + name for name in (
        'prepare-reviewed-references.mjs', 'prepare-reviewed-gap-references.mjs',
        'prepare-reviewed-full-references.mjs', 'build-reviewed-catalogue.mjs',
        'reviewed-reference-supplement/provenance.json',
        'reviewed-reference-supplement/supplement.js',
        'reviewed-reference-supplement/supplement.css',
        'reviewed-reference-supplement/session-current-body.input.json',
        'reviewed-reference-supplement/session-current-extraction.input.json',
        'reviewed-reference-supplement/session-current-hook-events.input.json',
        'reviewed-reference-supplement/settings-shortcuts-active.input.json',
        'reviewed-reference-supplement/settings-shortcuts-conflict.input.json',
    )
})
# Independently reviewed obligations, not a count inferred from the producer's
# own output. Every old state and every new full-window/component state executes.
REQUIRED_V4_CAPTURES = set(REQUIRED_CAPTURES) | {
    'library-populated',
    'session-current-body', 'session-current-extraction', 'session-current-hook-events',
    'target-review-preview-off', 'target-review-preview-live',
    'settings-shortcuts-active', 'settings-shortcuts-conflict',
    'hud-toolbar-560', 'hud-toolbar-500', 'hud-ruby-only', 'hud-lexical-error',
    'select-normal', 'select-long-list', 'preview-off', 'preview-live',
    'preview-paused', 'preview-permission', 'preview-unavailable', 'preview-exited',
    'session-extraction', 'session-hook-events',
}

REQUIRED_BEHAVIORS = {
    'UI-01-empty-library-no-fake-game',
    'UI-02-add-cancel-does-not-inject',
    'UI-03-association-one-confirmation-explicit-stream',
    'UI-04-manual-save-failure-retains-draft-through-navigation',
    'UI-05-settings-draft-test-apply-separated',
    'UI-06-settings-light-return-intent',
    'UI-07-wordcard-fixed-through-new-line',
    'UI-08-subscription-gap-recovers-with-snapshot',
    'UI-09-learning-open-does-not-auto-call-model',
    'UI-10-large-font-and-narrow-window-no-horizontal-overflow',
    'UI-11-release-bootstrap-excludes-controlled-transport',
    'UI-12-batch-program-review-preserves-all-and-language-confirmation',
    'UI-13-native-filedrop-opens-review-not-injection',
    'UI-14-parameter-zero-omit-and-draft-cas-identity',
    'UI-15-preference-draft-survives-settings-navigation',
    'UI-16-learning-new-line-and-explicit-ai-request-identity',
    'UI-17-history-pagination-retains-earlier-object',
    'UI-18-pool-version-and-connection-reference-separated',
    'UI-19-wordcard-fits-native-sized-viewport-after-source-change',
    'UI-20-native-url-to-production-frontend-role-contract',
    'UI-21-engine-order-and-enable-preserve-identity',
    'UI-22-schedule-defaults-and-boundary-errors',
    'UI-23-proxy-override-preserves-metadata-and-rejects-userinfo',
    'UI-24-game-glossary-draft-failure-and-stable-terms',
    'UI-25-production-entry-actually-mounts-without-test-transport',
    'UI-26-dictionary-entries-retain-headwords-and-hide-import-metadata',
    'UI-27-learning-hides-internal-unit-identity',
    'UI-28-japanese-punctuation-flow-and-200-percent-narrow-layout',
    'UI-29-hud-menu-opens-after-release-and-stays-open',
    'UI-30-hud-scrolls-only-reading-and-menu-regions',
    'UI-31-legacy-whitespace-gaps-preserve-reading-and-token-identity',
    'UI-32-applied-connection-control-does-not-apply-drafts',
    'UI-33-purpose-runtime-gate-is-independent-of-draft-and-connection-gates',
    'UI-34-credential-impact-cancel-and-failed-cleanup-retry-retains-operation',
    'UI-35-stale-runtime-confirmation-is-blocked-and-can-be-rechecked',
    'UI-36-terminal-session-state-precedes-saved-source-and-pause',
    'UI-37-history-queries-page-and-filter-before-projection-slice',
    'UI-38-history-revisions-current-location-and-unsaved-correction',
    'UI-39-study-tabs-fixed-object-and-explicit-model-actions',
    'UI-40-reading-logical-font-sizes-and-sakura-terminal-punctuation',
    'UI-41-model-default-templates-draft-restore-and-copy-cas',
    'UI-42-word-card-independent-freeze-explicit-action-and-generation',
    'UI-43-display-reference-rejects-wrong-menu-state',
    'UI-44-game-arguments-preserve-order-empty-values-and-cancel',
    'UI-45-settings-ten-categories-search-and-real-connection-reuse',
    'UI-46-target-list-and-mapping-stay-together-with-real-identity',
    'UI-47-batch-switch-cancel-and-failed-atomic-intent-retain-drafts',
    'UI-48-all-state-capture-contract-rejects-wrong-semantic-inputs',
    'UI-49-session-auto-translation-is-independent-of-purpose-draft',
    'UI-50-in-sheet-drop-preserves-dirty-and-batch-drafts',
    'UI-51-configuration-tabs-retain-draft-and-explicit-test-target',
    'UI-52-connection-copy-cancel-and-explicit-credential-reference',
    'UI-53-draft-removal-preserves-applied-history-and-credentials',
    'UI-54-model-enumeration-is-connection-test-not-save-or-apply',
    'UI-55-hud-actions-retain-source-and-patch-only-selected-preference',
    'UI-56-main-navigation-dismisses-unconfirmed-capture-and-retains-library-draft',
    'UI-57-running-game-only-entrypoints-and-one-confirmation',
}


def tree_digest(root):
    return hashlib.sha256(json.dumps(source_files(root), sort_keys=True).encode()).hexdigest()


def validate_freshness(report, nonce, started, completed):
    errors = []
    if report.get('executionNonce') != nonce:
        errors.append('Product capture receipt is not bound to this actual execution nonce')
    try:
        timestamp = datetime.fromisoformat(report['executedAt'].replace('Z', '+00:00')).timestamp()
        if not started <= timestamp <= completed:
            errors.append('Product capture receipt was not emitted during this actual execution')
    except (KeyError, TypeError, ValueError, AttributeError):
        errors.append('Product capture receipt lacks an actual execution timestamp')
    return errors


def validate_cargo_output(output):
    summaries = re.findall(r'^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;', output, re.M)
    errors = []
    if not summaries or sum(int(row[1]) for row in summaries) <= 0:
        errors.append('Production Rust tests must execute a nonzero number of tests')
    if any(row[0] != 'ok' or any(int(row[i]) for i in (2, 3, 4, 5)) for row in summaries):
        errors.append('Production Rust tests cannot fail, ignore, measure or filter required assertions')
    return sum(int(row[1]) for row in summaries), errors


def evidence_path(directory, relative):
    path = safe_path(directory, relative)
    if not path.is_file() or not path.stat().st_size:
        raise ValueError('Missing or empty actual evidence: ' + relative)
    return path


def inspect_window_title_case(case, artifact, build_exes, dosdevices):
    """Recompute summary facts from the owned WinAPI/probe raw byte artifacts."""
    observation = json.loads(artifact(case.get('observation')).read_text())
    scenarios = observation.get('scenarios') if isinstance(observation, dict) else None
    identity = case['id']
    name = identity.split('-', 3)[3]
    errors, checks, writes_total, producer_assertions = [], 0, 0, 0
    first_hashes = None

    def require(condition, message):
        nonlocal checks
        checks += 1
        if not condition:
            errors.append(identity + ': raw observation ' + message)

    def rows(data):
        result = []
        for line in data.splitlines():
            try:
                value = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                continue
            if isinstance(value, dict):
                result.append(value)
        return result

    def only(items, predicate):
        matches = [item for item in items if predicate(item)]
        if len(matches) != 1:
            raise ValueError(identity + ': raw fixture receipt missing or duplicated')
        return matches[0]

    if not isinstance(scenarios, list) or len(scenarios) != (2 if name.startswith('timeout-') else 1):
        raise ValueError(identity + ': original scenario observations missing or duplicated')
    for scenario in scenarios:
        fixture_bytes = artifact(scenario.get('fixture_stdout_artifact'), allow_empty=True).read_bytes()
        fixture_rows = rows(fixture_bytes)
        process = json.loads(artifact(scenario.get('fixture_process_artifact')).read_text())
        ready = only(fixture_rows, lambda row: row.get('kind') == 'ready')
        baseline = only(fixture_rows, lambda row: row.get('kind') == 'snapshot' and row.get('tag') == 'baseline')
        final = only(fixture_rows, lambda row: row.get('kind') == 'snapshot' and row.get('tag') == 'final')
        expected_exe = build_exes['fixture', case['fixture_arch']]
        expected_class = scenario.get('expected_fixture_class')
        image_path = bytes.fromhex(ready['path_utf16le_hex']).decode('utf-16le')
        fixture_class = bytes.fromhex(ready['class_utf16le_hex']).decode('utf-16le')
        require(process.get('parsed_stdout') == fixture_rows and process.get('stdout_bytes') == len(fixture_bytes), 'fixture process rows contradict stdout bytes')
        require(process.get('exit_code') == 0 and any(row.get('kind') == 'normal_exit' for row in fixture_rows), 'fixture did not exit normally')
        require(isinstance(process.get('command'), list) and str(expected_exe) in process['command'], 'fixture command does not use inspected PE')
        require(ready.get('image_sha256') == digest(expected_exe), 'loaded fixture EXE hash differs')
        mapping = dosdevices / (image_path[0].lower() + ':')
        require(len(image_path) > 3 and image_path[1:3] == ':\\' and mapping.is_symlink()
                and (mapping.resolve() / image_path[3:].replace('\\', '/')).resolve() == expected_exe.resolve(), 'loaded fixture image path differs from actual DOS drive mapping')
        require(isinstance(expected_class, str) and re.fullmatch(r'LunaOwnedApiFixture-[0-9a-f]{32}', expected_class) is not None and fixture_class == expected_class, 'random owned class differs')
        require(type(ready.get('pid')) is int and ready['pid'] > 0 and int(ready['hwnd']) > 0 and int(ready['creation']) > 0, 'fixture PID/HWND/lifetime is not exact')
        require(ready.get('bits') == (32 if case['fixture_arch'] == 'x86' else 64), 'actual fixture architecture differs')
        for snap in (baseline, final):
            require(all(snap.get(field) == ready.get(field) for field in ('pid', 'hwnd', 'class_utf16le_hex')), 'snapshot identity differs from owned fixture')
            rect = snap.get('rect')
            require(snap.get('is_visible') == 0 and snap.get('is_foreground') == 0 and isinstance(rect, list) and len(rect) == 4 and rect[2] < -10000, 'fixture visibility/focus/position is unsafe')
        require(scenario.get('fixture_ready') == ready and scenario.get('baseline') == baseline and scenario.get('final') == final, 'copied fixture observations contradict actual stdout')
        require(scenario.get('fixture_owner_events') == fixture_rows, 'owner events contradict actual stdout')
        original = hashlib.sha256(bytes.fromhex(baseline['caption_utf16le_hex'])).hexdigest()
        final_hash = hashlib.sha256(bytes.fromhex(final['caption_utf16le_hex'])).hexdigest()
        delta = final['wm_settext_count'] - baseline['wm_settext_count']
        require(type(delta) is int and delta >= 0, 'WM_SETTEXT counter moved backwards')
        writes_total += delta
        first_hashes = first_hashes or (original, final_hash)
        attempt = scenario['attempt']
        stdout = artifact(attempt.get('stdout_artifact'), allow_empty=True).read_bytes()
        stderr = artifact(attempt.get('stderr_artifact'), allow_empty=True).read_bytes().decode('utf-8', errors='replace')
        actual_probe = json.loads(artifact(attempt.get('process_artifact')).read_text())
        cleanup = scenario.get('cleanup', {})
        require(scenario.get('cleanup_confirmed') is True and case.get('cleanup_confirmed') is True,
                'owned child cleanup is not confirmed')
        for label, child in (('fixture', process), ('probe', actual_probe)):
            completion = child.get('completion', {})
            readers = completion.get('readers', {})
            require(completion.get('process_exited') is True and completion.get('exit_code') == child.get('exit_code')
                    and completion.get('setup_error') is None and completion.get('stdin_closed') is True
                    and completion.get('reader_completion_confirmed') is True,
                    label + ' process/pipe completion is not established')
            require(set(readers) == {'stdout', 'stderr'}, label + ' raw reader completion is missing')
            for stream, state in readers.items():
                expected_state = 'intentionally_closed' if label == 'probe' and stream == 'stdout' and name == 'stdout-failed-explicit-restore' else 'eof'
                require(state.get('alive') is False and state.get('state') == expected_state and state.get('error') is None,
                        label + ' raw reader is still alive, failed or lacks actual EOF')
            recorded = cleanup.get(label, {})
            require(recorded.get('confirmed') is True and recorded.get('errors') == []
                    and recorded.get('normal_fixture_exit') is True
                    and all(recorded.get(key) == value for key, value in completion.items()),
                    label + ' cleanup summary contradicts raw process completion')
        parsed = rows(stdout)
        command, code = actual_probe.get('command'), actual_probe.get('exit_code')
        require(attempt.get('parsed_stdout') == parsed and actual_probe.get('parsed_stdout') == parsed and actual_probe.get('stdout_bytes') == len(stdout), 'probe rows contradict actual stdout')
        require(attempt.get('command') == command and attempt.get('exit_code') == code and attempt.get('stderr') == stderr, 'probe invocation/exit/stderr differs from raw process evidence')
        require(isinstance(command, list) and str(build_exes['probe', case['probe_arch']]) in command and attempt.get('probe_exe_sha256') == digest(build_exes['probe', case['probe_arch']]), 'operator is not inspected Rust probe')
        index = command.index('--window-title-challenge')
        arguments = command[index + 1:]
        expected = [ready['hwnd'], str(ready['pid']), image_path, ready['creation']]
        if name == 'identity-pid-rejected': expected[1] = str(ready['pid'] + 1)
        if name == 'identity-path-rejected': expected[2] += '.wrong'
        if name == 'identity-creation-rejected': expected[3] = str(int(ready['creation']) + 1)
        require(len(arguments) == 7 and arguments[:4] == expected
                and re.fullmatch(r'LunaWindowProof-[A-Za-z0-9-]{17,96}', arguments[4]) is not None
                and arguments[5:] == [case['fixture_arch'], fixture_class], 'actual challenge arguments differ from exact owned identity/architecture/class/expected negative')
        restored = [row for row in parsed if row.get('status') == 'restored']
        proof_ready = [row for row in parsed if row.get('status') == 'ready']
        same = original == final_hash
        if name == 'ansi-prewrite-rejected':
            require(ready.get('is_unicode', baseline.get('is_unicode')) == 0 and code == 0 and delta == 0 and same and not proof_ready and not restored and any(row.get('status') == 'unavailable' and row.get('errorCode') == 'window_title_proof_requires_unicode' for row in parsed), 'ANSI refusal mutated title or was not typed unavailable before write')
        elif name in {'unicode-exact-restore', 'observer-eof-restores'}:
            require(code == 0 and len(proof_ready) == 1 and len(restored) == 1 and same and delta == 2, 'Unicode/EOF exact roundtrip failed')
            require(len(restored) == 1 and restored[0].get('titleRestored') is True and restored[0].get('identityVerified') is True and restored[0].get('requested') is (name != 'observer-eof-restores'), 'restoration receipt fields contradict actual operation')
        elif name == 'observer-thirdparty-preserved':
            third = only(fixture_rows, lambda row: row.get('kind') == 'snapshot' and row.get('tag') == 'thirdparty-visible')
            require(code != 0 and len(proof_ready) == 1 and not restored and 'title_restore_failed' in stderr and delta == 1 and final['caption_utf16le_hex'] == third['caption_utf16le_hex'] and not same, 'third-party title was overwritten or acknowledged')
        elif name.startswith(('identity-', 'caption-')):
            require(code != 0 and delta == 0 and same and not restored and not proof_ready, 'preflight failure wrote title or acknowledged success')
            if name.startswith('caption-'):
                require(('caption_truncated' if 'truncated' in name else 'caption_invalid_utf16') in stderr, 'strict original caption error was lost')
        elif name == 'handler-rejected-explicit-restore':
            require(code != 0 and not proof_ready and not restored and delta == 2 and same
                    and all(text in stderr for text in ('window_title_update_rejected', 'title_restore_failed'))
                    and 'original_title_restored' not in stderr,
                    'two handler refusals must retain exact caption and explicit failed restore without a success ACK or retry')
        elif name == 'stdout-failed-explicit-restore':
            require(code != 0 and delta == 2 and same and all(text in stderr for text in ('proof output failed', 'original_title_restored')), 'broken ready output left an unconfirmed mutation')
        elif name == 'restore-invalid-utf16-not-acknowledged':
            require(code != 0 and len(proof_ready) == 1 and not restored and delta == 2 and final['caption_utf16le_hex'] == '00d8' and all(text in stderr for text in ('title_restore_failed', 'invalid_utf16')), 'invalid UTF16 restore was silently accepted or retried')
        elif name.startswith('timeout-'):
            claimed = 'original_title_restored' in stderr or any(row.get('titleRestored') is True for row in restored)
            require(code != 0 and 'window_title_update_failed' in stderr and (not claimed or same), 'timed-out write falsely reported restoration')
            if scenario.get('mode') == 'slow650':
                require(same and delta == 2, '650ms delayed write was not restored exactly')
            else:
                require(scenario.get('mode') == 'slow2400' and not claimed and delta <= 2 and ('title_restore_failed' in stderr or 'unconfirmed' in stderr), '2400ms over-budget write lacked explicit unconfirmed state')
        count = scenario.get('assertion_count')
        require(type(count) is int and count > 0, 'original maintained assertion record is missing')
        producer_assertions += count if type(count) is int and count > 0 else 0
    require(case.get('wm_settext_count') == writes_total, 'case WM_SETTEXT summary contradicts original counters')
    require((case.get('original_caption_sha256'), case.get('final_caption_sha256')) == first_hashes, 'case caption hashes contradict raw UTF16 bytes')
    require(case.get('assertion_count') == producer_assertions, 'case assertion count contradicts original scenarios')
    return errors, checks


def validate_window_title_proof_report(root, directory, nonce):
    """Inspect actual WinAPI fixture receipts; never classify them as game tests."""
    report_path = evidence_path(directory, 'receipt.json')
    report = json.loads(report_path.read_text())
    if not isinstance(report, dict):
        raise ValueError('WinAPI receipt must be an object')
    errors, artifacts = [], []
    if (type(report.get('schema_version')) is not int or report['schema_version'] != 1
            or report.get('adapter_id') != WINDOW_TITLE_PROOF_CHECK
            or report.get('execution_nonce') != nonce
            or report.get('scope') != WINDOW_TITLE_PROOF_SCOPE
            or report.get('native_game_verified') is not False
            or report.get('source_before_after_equal') is not True):
        errors.append('WinAPI receipt identity, nonce, source stability or fixture-only scope differs')
    if report.get('status') != 'passed' or report.get('errors') != []:
        errors.append('Actual maintained WinAPI producer reported a failure')
    source_hashes = report.get('source_sha256', {})
    dosdevices_value = report.get('dosdevices_path')
    if not isinstance(dosdevices_value, str) or not Path(dosdevices_value).is_absolute() or not Path(dosdevices_value).is_dir():
        raise ValueError('WinAPI receipt must identify the actual read-only DOS drive mapping directory')
    dosdevices = Path(dosdevices_value)
    if not isinstance(source_hashes, dict) or set(source_hashes) != WINDOW_TITLE_PROOF_SOURCES:
        errors.append('WinAPI receipt must bind the exact seven maintained source inputs')
    else:
        for path, expected in source_hashes.items():
            if digest(safe_path(root, path)) != expected:
                errors.append('WinAPI maintained source differs: ' + path)

    def artifact(row, allow_empty=False):
        if not isinstance(row, dict):
            raise ValueError('WinAPI artifact must provide path and sha256')
        path = safe_path(directory, row.get('path', ''))
        if not path.is_file() or (not allow_empty and not path.stat().st_size):
            raise ValueError('WinAPI artifact is missing or unexpectedly empty')
        if row.get('sha256') != digest(path):
            raise ValueError('WinAPI artifact hash differs: ' + str(row.get('path')))
        artifacts.append({'path': str(path.relative_to(root)), 'sha256': digest(path)})
        return path

    build_hashes, build_keys, build_exes = {}, [], {}
    builds = report.get('builds', [])
    if not isinstance(builds, list):
        raise ValueError('WinAPI builds must be a list')
    for build in builds:
        if not isinstance(build, dict):
            raise ValueError('WinAPI build must be an object')
        kind, arch = build.get('kind'), build.get('probe_arch')
        key = (kind, arch)
        build_keys.append(key)
        command = build.get('command')
        if (kind not in {'probe', 'fixture'} or arch not in WINDOW_TITLE_PROOF_TARGETS
                or build.get('target') != WINDOW_TITLE_PROOF_TARGETS.get(arch)
                or build.get('exit_code') != 0 or isinstance(build.get('exit_code'), bool)
                or not isinstance(command, list) or not command or not all(isinstance(v, str) for v in command)):
            errors.append('WinAPI build architecture, actual command or exit status differs')
        elif kind == 'probe' and (Path(command[0]).name != 'cargo' or not {'build', '--locked', '--offline', '-p', 'lunahook-host', '--example', 'target_probe', '--target', build['target']} <= set(command)):
            errors.append('WinAPI challenge operator must be built from maintained Rust target_probe')
        artifact(build.get('log'))
        exe_path = artifact(build.get('exe'))
        build_exes[key] = exe_path
        exe = exe_path.read_bytes()
        offset = int.from_bytes(exe[60:64], 'little') if len(exe) >= 64 else len(exe)
        machine = int.from_bytes(exe[offset + 4:offset + 6], 'little')
        if (exe[:2] != b'MZ' or exe[offset:offset + 4] != b'PE\0\0'
                or machine != {'x86': 0x14c, 'x64': 0x8664}.get(arch)):
            errors.append('WinAPI executable PE machine differs from required architecture')
        build_hashes[key] = build['exe']['sha256']
    if len(build_keys) != 4 or set(build_keys) != {(kind, arch) for kind in ('probe', 'fixture') for arch in WINDOW_TITLE_PROOF_TARGETS}:
        errors.append('WinAPI receipt must include both actual Rust probes and both fixture architectures')
    cases = report.get('cases', [])
    if not isinstance(cases, list):
        raise ValueError('WinAPI cases must be a list')
    identities, assertion_count, inspected_assertions = [], 0, 0
    for case in cases:
        if not isinstance(case, dict):
            raise ValueError('WinAPI case must be an object')
        identity = case.get('id')
        identities.append(identity)
        count = case.get('assertion_count')
        fixture, probe = case.get('fixture_arch'), case.get('probe_arch')
        if (not isinstance(identity, str) or not identity.startswith(f'WTPA-{fixture}-{probe}-')
                or case.get('status') != 'passed' or type(count) is not int or count <= 0
                or case.get('exact_identity_guard') is not True
                or case.get('probe_exe_sha256') != build_hashes.get(('probe', probe))
                or case.get('fixture_exe_sha256') != build_hashes.get(('fixture', fixture))):
            errors.append('WinAPI case identity, assertion count, guard or actual EXE binding differs: ' + str(identity))
        assertion_count += count if type(count) is int and count > 0 else 0
        writes = case.get('wm_settext_count')
        if type(writes) is not int or writes < 0:
            errors.append('WinAPI case lacks observed WM_SETTEXT count: ' + str(identity))
        elif isinstance(identity, str) and (identity.endswith('prewrite-rejected') or '-identity-' in identity) and writes != 0:
            errors.append('WinAPI prewrite/identity rejection mutated a fixture title: ' + identity)
        elif isinstance(identity, str) and identity.endswith(('unicode-exact-restore', 'observer-eof-restores')) and writes < 2:
            errors.append('WinAPI successful challenge/restore lacks observed title writes: ' + identity)
        original, final = case.get('original_caption_sha256'), case.get('final_caption_sha256')
        if not all(isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) for value in (original, final)):
            errors.append('WinAPI original/final caption evidence is missing: ' + str(identity))
        elif isinstance(identity, str) and identity.endswith(('unicode-exact-restore', 'observer-eof-restores')) and original != final:
            errors.append('WinAPI successful restore did not retain the exact original caption: ' + identity)
        case_artifacts = case.get('artifacts')
        if not isinstance(case_artifacts, list) or not case_artifacts:
            errors.append('WinAPI case has no raw call/fixture evidence: ' + str(identity))
        else:
            for row in case_artifacts:
                artifact(row, allow_empty=True)
        raw_errors, raw_checks = inspect_window_title_case(case, artifact, build_exes, dosdevices)
        errors.extend(raw_errors)
        inspected_assertions += raw_checks
    if len(identities) != len(REQUIRED_WINDOW_TITLE_PROOF_CASES) or set(identities) != REQUIRED_WINDOW_TITLE_PROOF_CASES:
        errors.append('WinAPI required maintained cases are missing, duplicated or unknown')
    if type(report.get('case_count')) is not int or report['case_count'] != len(identities):
        errors.append('WinAPI case count differs from actual inspected case rows')
    if type(report.get('assertion_count')) is not int or report['assertion_count'] != assertion_count:
        errors.append('WinAPI assertion count differs from actual inspected case rows')
    return errors, {'test_count': len(identities), 'assertion_count': assertion_count,
                    'independently_recomputed_assertion_count': inspected_assertions,
                    'native_game_verified': False, 'artifacts': artifacts,
                    'winapi_receipt': str(report_path.relative_to(root)), 'winapi_receipt_sha256': digest(report_path)}


def load_reference_catalogue(root, check_id='product-ui-capture'):
    """Inspect reviewed immutable design inputs; never generate expected images."""
    if check_id not in PRODUCT_CAPTURE_CHECKS:
        raise ValueError('Unknown actual capture adapter')
    reviewed = check_id == 'product-ui-capture-v4'
    catalogue_ref = REFERENCE_CATALOGUE_V4 if reviewed else REFERENCE_CATALOGUE
    required = REQUIRED_V4_CAPTURES if reviewed else set(REQUIRED_CAPTURES)
    prefix = 'docs/design/first-product-v1/acceptance-v4/' if reviewed else 'docs/design/first-product-v1/acceptance-v3/'
    path = safe_path(root, catalogue_ref)
    baseline = json.loads(safe_path(root, 'docs/development/iteration-baseline.json').read_text())
    pins = [entry for entry in baseline.get('files', []) if entry.get('path') == catalogue_ref]
    if len(pins) != 1 or pins[0].get('sha256') != digest(path):
        raise ValueError('Design catalogue differs from independently reviewed baseline registration')
    catalogue = json.loads(path.read_text())
    if (not isinstance(catalogue, dict) or catalogue.get('schemaVersion') != (4 if reviewed else 3)
            or catalogue.get('status') != 'registered_independently_reviewed'
            or catalogue.get('pixelmatch') != {'threshold': 0.1, 'includeAA': True, 'maxRatio': 0}):
        raise ValueError('Missing independently registered same-state design catalogue')
    states = catalogue.get('states', [])
    if not isinstance(states, list) or len(states) != len(required) or {row.get('id') for row in states} != required:
        raise ValueError('Design catalogue must register the complete independently required state inventory' if reviewed else 'Design catalogue must register all fifteen distinct states')
    frozen = catalogue.get('frozenFiles', {})
    if not isinstance(frozen, dict) or not frozen:
        raise ValueError('Design catalogue lacks independently frozen sources')
    if reviewed:
        authority_ref = 'docs/development/design-approval-20261019.json'
        authority_path = safe_path(root, authority_ref)
        authority_pins = [entry for entry in baseline.get('files', []) if entry.get('path') == authority_ref]
        if len(authority_pins) != 1 or authority_pins[0].get('sha256') != digest(authority_path):
            raise ValueError('Current design approval differs from the independent authority baseline')
        approved = json.loads(authority_path.read_text()).get('source_files_sha256')
        if (not isinstance(approved, dict) or len(approved) != 16
                or catalogue.get('approvedDesignSources') != approved
                or any(frozen.get(relative) != sha for relative, sha in approved.items())):
            raise ValueError('Reviewed capture must bind all sixteen current approved design sources')
        components = {'hud-toolbar-560', 'hud-toolbar-500', 'hud-ruby-only', 'hud-lexical-error',
                      'select-normal', 'select-long-list', 'preview-off', 'preview-live',
                      'preview-paused', 'preview-permission', 'preview-unavailable', 'preview-exited',
                      'session-extraction', 'session-hook-events'}
        if any(row.get('surfaceKind') != ('component' if row.get('id') in components else 'full-window') for row in states):
            raise ValueError('Component captures and complete native-client window states must remain distinct')
        if not V4_REFERENCE_PRODUCER_INPUTS.issubset(frozen):
            raise ValueError('Independent reference producer closure is incomplete')
    # Keep the original logical source domain guard. Historical resolution may
    # select same-byte locations, never enlarge the independent producer set.
    for relative, sha in frozen.items():
        external_input = reviewed and relative in V4_REFERENCE_PRODUCER_INPUTS
        if (not (relative.startswith('docs/design/first-product-v1/') or external_input)
                or not re.fullmatch(r'[0-9a-f]{64}', str(sha))):
            raise ValueError('Frozen design input/hash differs: ' + str(relative))
    source_version = load_source_version(root, catalogue, digest(path)) if reviewed else None
    for relative, sha in frozen.items():
        external_input = reviewed and relative in V4_REFERENCE_PRODUCER_INPUTS
        physical = source_version.resolve(relative) if source_version else safe_path(root, relative)
        if digest(physical) != sha:
            raise ValueError('Frozen design input/hash differs: ' + str(relative))
        if external_input:
            source_pins = [entry for entry in baseline.get('files', []) if entry.get('path') == relative]
            if len(source_pins) != 1 or source_pins[0].get('sha256') != sha:
                raise ValueError('External reference producer differs from independent authority pin: ' + relative)
    adapter_ref = catalogue.get('designAdapterManifest', '')
    adapter_path = safe_path(root, adapter_ref)
    adapter_hash = digest(adapter_path)
    adapter = json.loads(adapter_path.read_text())
    if (frozen.get(adapter_ref) != adapter_hash or not isinstance(adapter, dict)
            or not isinstance(adapter.get('files'), dict) or not adapter['files']
            or any(frozen.get(relative) != sha for relative, sha in adapter['files'].items())):
        raise ValueError('Design adapter manifest is not independently frozen')
    for state in states:
        sources = state.get('designSourceHashes', {})
        if (not isinstance(sources, dict) or not sources
                or any(frozen.get(key) != sha for key, sha in sources.items())
                or state.get('designAdapterHash') != adapter_hash or not state.get('dataState')
                or not state.get('clientBoundary') or not isinstance(state.get('nativeExclusions'), list)):
            raise ValueError('Incomplete independent design provenance: ' + state['id'])
        if 'underlay' in state:
            underlay = state['underlay']
            if (not isinstance(underlay, dict) or frozen.get(underlay.get('file')) != underlay.get('sha256')
                    or not re.fullmatch(r'[0-9a-f]{64}', str(underlay.get('sha256')))
                    or not isinstance(underlay.get('size'), list) or len(underlay['size']) != 2
                    or any(type(n) is not int or n <= 0 for n in underlay['size'])):
                raise ValueError('Unfrozen or invalid common underlay: ' + state['id'])
            if underlay.get('placement') == 'center / cover':
                # Independent native-content examples share this explicit CSS
                # input on both sides, rather than pretending to be a desktop
                # crop. The reference/candidate PNGs themselves are never scaled.
                if 'scale' in underlay or 'sampleOrigin' in underlay:
                    raise ValueError('Common underlay cannot mix cover and fixed sampling')
            elif ('placement' in underlay or underlay.get('scale') != 1
                    or not isinstance(underlay.get('sampleOrigin'), list) or len(underlay['sampleOrigin']) != 2
                    or any(type(n) not in (int, float) or not math.isfinite(n) or n < 0 for n in underlay['sampleOrigin'])):
                raise ValueError('Unfrozen or invalid common underlay sampling: ' + state['id'])
        variants = state.get('variants', [])
        if not isinstance(variants, list) or not variants or len({row.get('variantId') for row in variants}) != len(variants):
            raise ValueError('Missing or duplicate design variants: ' + state['id'])
        for variant in variants:
            reference = variant.get('reference', '')
            if (not reference.startswith(prefix)
                    or not reference.endswith('.png') or frozen.get(reference) != variant.get('referenceSha256')):
                raise ValueError('Unfrozen design variant reference: ' + state['id'])
            conditions = variant.get('requiredConditions', {})
            size, viewport = variant.get('expectedSize'), variant.get('viewport', {})
            if (not isinstance(size, list) or len(size) != 2 or any(type(n) is not int or n <= 0 for n in size)
                    or set(viewport) != {'width', 'height'} or any(type(n) is not int or n <= 0 for n in viewport.values())
                    or variant.get('deviceScaleFactor') != 1 or not variant.get('selector')
                    or not isinstance(variant.get('expectedBounds'), dict) or not isinstance(variant.get('scrollTop'), list)
                    or not isinstance(conditions, dict) or not {'windowRole', 'viewport', 'theme', 'transparency'} <= set(conditions)):
                raise ValueError('Incomplete design geometry/state: ' + state['id'])
            bounds = variant['expectedBounds']
            if (set(bounds) != {'x', 'y', 'width', 'height'}
                    or any(type(n) not in (int, float) or not math.isfinite(n) for n in bounds.values())
                    or min(bounds['x'], bounds['y']) < 0 or min(bounds['width'], bounds['height']) <= 0
                    or bounds['x'] + bounds['width'] > viewport['width'] + 0.001
                    or bounds['y'] + bounds['height'] > viewport['height'] + 0.001
                    # Match pinned Playwright's enclosingIntRect rounding.
                    or size != [math.ceil(bounds['x'] + bounds['width'] - 0.001) - math.floor(bounds['x'] + 0.001),
                                math.ceil(bounds['y'] + bounds['height'] - 0.001) - math.floor(bounds['y'] + 0.001)]
                    or conditions['viewport'] != viewport):
                raise ValueError('Contradictory or out-of-bounds design client geometry: ' + state['id'])
            if 'underlay' in state and state['underlay'].get('placement') != 'center / cover':
                underlay = state['underlay']
                if any(underlay['sampleOrigin'][index] + viewport[key] > underlay['size'][index]
                       for index, key in enumerate(('width', 'height'))):
                    raise ValueError('Design common underlay does not cover the client viewport: ' + state['id'])
    return {row['id']: row for row in states}, digest(path)


def _contains(actual, expected):
    if isinstance(expected, dict):
        return isinstance(actual, dict) and all(key in actual and _contains(actual[key], value) for key, value in expected.items())
    return actual == expected


def validate_capture_conditions(identity, row, state, spec):
    errors = []
    actual = row.get('captureConditions', {})
    for key, expected in [('viewport', spec['viewport']), ('deviceScaleFactor', spec['deviceScaleFactor']),
                          ('selector', spec['selector']), ('bounds', spec['expectedBounds'])]:
        if actual.get(key) != expected:
            errors.append(identity + ': actual ' + key + ' differs from frozen client boundary/state; expected=' + json.dumps(expected, sort_keys=True) + ' actual=' + json.dumps(actual.get(key), sort_keys=True))
    expected_scroll, actual_scroll = spec['scrollTop'], actual.get('scrollTop')
    if actual_scroll != expected_scroll:
        if not isinstance(actual_scroll, list):
            errors.append(identity + ': scroll regions expected=' + json.dumps(expected_scroll, sort_keys=True) + ' actual=' + json.dumps(actual_scroll, sort_keys=True))
        else:
            if len(expected_scroll) != len(actual_scroll):
                errors.append(f'{identity}: scroll region count expected={len(expected_scroll)} actual={len(actual_scroll)}')
            for index in range(max(len(expected_scroll), len(actual_scroll))):
                expected = expected_scroll[index] if index < len(expected_scroll) else None
                observed = actual_scroll[index] if index < len(actual_scroll) else None
                if not isinstance(expected, dict) or not isinstance(observed, dict):
                    if expected != observed:
                        errors.append(f'{identity}: scroll[{index}] expected=' + json.dumps(expected, sort_keys=True) + ' actual=' + json.dumps(observed, sort_keys=True))
                    continue
                for field in sorted(set(expected) | set(observed)):
                    if field not in expected or field not in observed or expected[field] != observed[field]:
                        errors.append(f'{identity}: scroll[{index}].{field} expected=' + json.dumps(expected.get(field, '<missing>'), sort_keys=True) + ' actual=' + json.dumps(observed.get(field, '<missing>'), sort_keys=True))
    query = state.get('dataState', {}).get('componentInputs', {}).get('supplement', {}).get('query')
    if query is not None and actual.get('sessionRecordQuery') != query:
        errors.append(identity + ': actual session/dataset/filter/order/pageSize/cursor query differs from independent fixture; expected=' + json.dumps(query, sort_keys=True) + ' actual=' + json.dumps(actual.get('sessionRecordQuery'), sort_keys=True))
    verified = actual.get('verifiedConditions')
    conditions = dict(spec['requiredConditions'])
    source_font = conditions.pop('sourceFont', None)
    menu = conditions.pop('hudMenu', None)
    fields = conditions.pop('fields', None)
    if not _contains(verified, conditions):
        errors.append(identity + ': actual rendered preconditions differ from registered design state')
    if source_font and (not verified or not verified.get('sourceFonts') or any(font.get('size') != source_font for font in verified['sourceFonts'])):
        errors.append(identity + ': rendered font differs')
    if menu and (not verified or verified.get('visibleMenus') != (1 if menu == 'open' else 0) or verified.get('menuTriggerExpanded') != ('true' if menu == 'open' else 'false')):
        errors.append(identity + ': rendered menu state differs')
    if fields is not None:
        expected_fields = [{key: field[key] for key in ('role', 'name', 'value', 'options') if key in field} for field in fields]
        if not verified or verified.get('fields') != expected_fields:
            errors.append(identity + ': actual form fields differ')
    if 'underlay' in state and actual.get('underlay') != state['underlay']:
        errors.append(identity + ': common backdrop input differs')
    if row.get('expectedSize') != spec['expectedSize']:
        errors.append(identity + ': emitted dimensions differ from registered reference')
    return errors


def validate_capture_report(root, directory, report, recomputed, exit_code, check_id='product-ui-capture'):
    """Inspect all captures, including additions; a passed string is never sufficient."""
    errors = []
    catalogue, catalogue_hash = load_reference_catalogue(root, check_id)
    if report.get('referenceCatalogueSha256') != catalogue_hash:
        errors.append('Product capture receipt differs from the independently registered catalogue')
    if check_id == 'product-ui-capture-v4' and report.get('designSourceVersion') != load_source_version(root).receipt:
        errors.append('Product capture source-version receipt differs from independently inspected historical and active source identities')
    if report.get('version') != 1:
        errors.append('Unknown product UI receipt version')
    behavior, captures = report.get('behavior', []), report.get('captures', [])
    behavior_ids = [row.get('id') for row in behavior]
    if len(set(behavior_ids)) != len(behavior_ids) or not REQUIRED_BEHAVIORS <= set(behavior_ids):
        errors.append('Actual product UI behavior inventory is missing or duplicated')
    if any(row.get('status') not in {'passed', 'failed'} or row.get('skipped') for row in behavior):
        errors.append('Product UI behavior assertions must execute without skip')
    behavior_status = 'passed' if behavior and all(row.get('status') == 'passed' for row in behavior) and not report.get('browserErrors') else 'failed'
    capture_ids = [row.get('id') for row in captures]
    if len(set(capture_ids)) != len(capture_ids) or set(catalogue) != set(capture_ids):
        errors.append('Actual product UI capture inventory is missing or duplicated')
    flat = []
    blocked_ids = set()
    declared_keys = set()
    for parent in captures:
        identity = parent.get('id')
        state = catalogue.get(identity)
        if state is None:
            errors.append(str(identity) + ': unregistered design state')
            continue
        children = [parent, *parent.get('variants', [])]
        expected = state['variants']
        if [child.get('variantId') for child in children] != [variant['variantId'] for variant in expected]:
            errors.append(identity + ': actual variants differ from complete registered inventory')
        if parent.get('status') == BLOCKED_REFERENCE_STATUS:
            blocked_ids.add(identity)
            registered_code = HISTORICAL_CAPTURE_BLOCKS.get(identity) if check_id == 'product-ui-capture-v4' else None
            if registered_code is None:
                errors.append(identity + ': unregistered historical blocked state cannot bypass capture')
            for child, spec in zip(children, expected):
                blocked = child.get('blocked', {})
                if (child.get('id') != identity or child.get('status') != BLOCKED_REFERENCE_STATUS
                        or not isinstance(blocked, dict) or set(blocked) != {'code', 'replacementReference', 'reason'}
                        or blocked.get('code') != registered_code or blocked.get('replacementReference') is not None
                        or not isinstance(blocked.get('reason'), str) or not blocked.get('reason')
                        or child.get('reference') != spec['reference'] or child.get('referenceSha256') != spec['referenceSha256']
                        or child.get('expectedSize') != spec['expectedSize']
                        or any(key in child for key in ('baselineCopy', 'candidate', 'diff', 'actualSize', 'differentPixels', 'ratio', 'captureConditions'))):
                    errors.append(identity + ': blocked receipt contradicts its exact frozen obligation or invents captured facts')
            errors.append(identity + ': ' + BLOCKED_REFERENCE_STATUS + '; original reference obligation remains unsatisfied')
            continue
        for child in children:
            if child.get('status') == BLOCKED_REFERENCE_STATUS or 'blocked' in child:
                errors.append(identity + ': partial or forged blocked status cannot bypass required variants')
                blocked_ids.add(identity)
                continue
            key = (identity, child.get('variantId'))
            declared_keys.add(key)
            spec = next((variant for variant in expected if variant['variantId'] == child.get('variantId')), None)
            if spec is None:
                errors.append(identity + ': unknown variant')
                continue
            errors.extend(validate_capture_conditions(identity + '/' + str(child.get('variantId')), child, state, spec))
            flat.append((identity, child, spec))
    comparisons = {(row['id'], row.get('variantId')): row for row in recomputed}
    if len(comparisons) != len(recomputed) or set(comparisons) != declared_keys:
        errors.append('Independent PNG comparison inventory differs from emitted capture inventory')
    actual_statuses = []
    per_state = {}
    for identity, row, spec in flat:
        reference = row.get('reference', '')
        reference_file = safe_path(root, reference)
        if reference != spec['reference'] or row.get('referenceSha256') != spec['referenceSha256']:
            errors.append(identity + ': reference differs from registered design state/variant')
        baseline = evidence_path(directory, row.get('baselineCopy', ''))
        candidate = evidence_path(directory, row.get('candidate', ''))
        diff = evidence_path(directory, row.get('diff', ''))
        if len({baseline, candidate, diff, reference_file}) != 4:
            errors.append(identity + ': reference, baseline, candidate and diff must be separate files')
        if digest(reference_file) != row.get('referenceSha256') or baseline.read_bytes() != reference_file.read_bytes():
            errors.append(identity + ': source reference hash or copied baseline differs')
        if digest(candidate) != row.get('candidateSha256'):
            errors.append(identity + ': actual candidate PNG hash differs')
        if row.get('diffSha256') and digest(diff) != row['diffSha256']:
            errors.append(identity + ': actual diff PNG hash differs')
        actual = comparisons.get((identity, row.get('variantId')), {})
        if actual.get('diffSha256') != digest(diff):
            errors.append(identity + ': actual diff does not match independent pixel comparison')
        for key in ['expectedSize', 'actualSize', 'differentPixels', 'ratio']:
            value = row.get(key)
            if value != actual.get(key) or isinstance(value, float) and not math.isfinite(value):
                errors.append(identity + ': emitted ' + key + ' differs from independent pixel comparison')
        if row.get('pixelmatchThreshold') != 0.1 or row.get('includeAA') is not True:
            errors.append(identity + ': pixel comparator options differ from frozen contract')
        if type(row.get('maxRatio')) not in {int, float} or row['maxRatio'] != 0:
            errors.append(identity + ': unreviewed visual threshold must stay at registered zero')
        passed = (actual.get('expectedSize') == actual.get('actualSize') and actual.get('differentPixels') == 0
                  and not validate_capture_conditions(identity, row, catalogue[identity], spec)
                  and not row.get('referencePremiseMismatch'))
        status = 'passed' if passed else 'failed'
        actual_statuses.append(status)
        per_state.setdefault(identity, []).append(status)
        # Parent status covers its own PNG and every required scroll variant.
        if row.get('variants') is None and row.get('status') != status:
            errors.append(identity + ': emitted visual result contradicts actual PNGs')
    for parent in captures:
        if parent.get('status') == BLOCKED_REFERENCE_STATUS:
            continue
        status = 'passed' if per_state.get(parent.get('id')) and all(value == 'passed' for value in per_state[parent['id']]) else 'failed'
        if parent.get('status') != status:
            errors.append(str(parent.get('id')) + ': aggregate visual result contradicts required variants')
    visual_status = 'passed' if (captures and actual_statuses and not blocked_ids
                                 and len(capture_ids) == len(catalogue) and set(capture_ids) == set(catalogue)
                                 and set(comparisons) == declared_keys
                                 and all(status == 'passed' for status in actual_statuses)) else 'failed'
    status = 'passed' if behavior_status == visual_status == 'passed' else 'failed'
    for key, actual in [('behaviorStatus', behavior_status), ('visualStatus', visual_status), ('status', status)]:
        if report.get(key) != actual:
            errors.append('Product UI ' + key + ' contradicts actual assertions and PNGs')
    if exit_code != (0 if status == 'passed' else 1):
        errors.append('Product UI process exit code contradicts actual assertions and PNGs')
    if behavior_status != 'passed':
        errors.append('Actual production-component behavior checks failed')
    if visual_status != 'passed':
        errors.append('Actual product visual comparison failed; current design references remain unchanged')
    return errors, behavior_status, visual_status


def run_product_check(root, check_id, output_root, runtime_inputs=None):
    root = root.resolve()
    output_root = output_root.resolve()
    if not output_root.is_relative_to(root / '.build'):
        raise ValueError('Product execution evidence must be under repository .build')
    registry = {**PRODUCT_CAPTURE_CHECKS, **PRODUCT_TEST_CHECKS, **PRODUCT_RUNTIME_CHECKS}
    if check_id not in registry:
        raise ValueError('Unregistered actual product adapter: ' + check_id)
    run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '-' + uuid.uuid4().hex[:10]
    directory = output_root / (run_id + '-' + check_id)
    directory.mkdir(parents=True, exist_ok=False)
    before = tree_digest(root)
    command = list(registry[check_id])
    if check_id in PRODUCT_CAPTURE_CHECKS:
        command += ['--output', str(directory / 'capture')]
    started = time.monotonic()
    started_wall = time.time()
    nonce = uuid.uuid4().hex
    if check_id == 'product-cp01-reading-tests':
        command += ['--output', str(directory / 'component'), '--nonce', nonce]
    if check_id == 'product-settings-controls-tests':
        command += ['--output', str(directory / 'settings'), '--nonce', nonce]
    if check_id == 'product-preview-visual-tests':
        command += ['--output', str(directory / 'preview'), '--nonce', nonce]
    runtime_input_error = None
    if check_id == WINDOW_TITLE_PROOF_CHECK:
        command += ['--output', str(directory / 'winapi'), '--execution-nonce', nonce]
    if check_id in PRODUCT_RUNTIME_CHECKS:
        if check_id != RUNTIME_STABILITY_CHECK:
            raise ValueError('Runtime registry entry has no actual execution dispatcher')
        try:
            if not runtime_inputs: raise ValueError('Packaged stability requires explicit --runtime-inputs')
            source_input = safe_path(root, str(runtime_inputs))
            from runtime_stability import load_inputs
            load_inputs(source_input, root)
            (directory / 'runtime-inputs.json').write_bytes(source_input.read_bytes())
        except (OSError, ValueError, TypeError, KeyError) as error:
            runtime_input_error = str(error)
        command += ['--inputs', str(directory / 'runtime-inputs.json'), '--output', str(directory / 'runtime'), '--execution-nonce', nonce]
    try:
        if runtime_input_error: raise ValueError(runtime_input_error)
        executed = subprocess.run(command, cwd=root, env={**os.environ, 'LUNA_PRODUCT_EXECUTION_NONCE': nonce}, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=7200 if check_id in PRODUCT_RUNTIME_CHECKS else 900)
        stdout, stderr, code = executed.stdout, executed.stderr or '', executed.returncode
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        stdout, stderr, code = '', str(error), -1
    completed_wall = time.time()
    log = directory / 'execution.log'
    log.write_text(stdout + '\n' + stderr)
    errors, details = [], {}
    if before != tree_digest(root):
        errors.append('Source tree changed while actual product checks were running; rerun on stable sources')
    if check_id in PRODUCT_CAPTURE_CHECKS:
        try:
            report_path = evidence_path(directory / 'capture', 'verification.json')
            report = json.loads(report_path.read_text())
            errors.extend(validate_freshness(report, nonce, started_wall, completed_wall))
            # Validate file paths/hashes before a Node decoder touches untrusted report paths.
            for row in [child for parent in report.get('captures', []) for child in [parent, *parent.get('variants', [])]]:
                if row.get('status') == BLOCKED_REFERENCE_STATUS:
                    continue
                for key in ['baselineCopy', 'candidate', 'diff']:
                    evidence_path(report_path.parent, row.get(key, ''))
            comparison_command = ['node', 'scripts/contracts/compare_product_capture.mjs', str(report_path)]
            comparison = subprocess.run(comparison_command, cwd=root, capture_output=True, text=True, timeout=120)
            comparison_log = directory / 'comparison.log'
            comparison_log.write_text(comparison.stdout + '\n' + comparison.stderr)
            if comparison.returncode:
                raise ValueError('Independent PNG recomputation failed; see ' + str(comparison_log.relative_to(root)))
            recomputed = json.loads(comparison.stdout)
            recompute_path = directory / 'recomputed.json'
            recompute_path.write_text(json.dumps(recomputed, indent=2) + '\n')
            capture_errors, behavior_status, visual_status = validate_capture_report(root, report_path.parent, report, recomputed, code, check_id)
            errors.extend(capture_errors)
            artifacts = []
            for row in [child for parent in report['captures'] for child in [parent, *parent.get('variants', [])]]:
                if row.get('status') == BLOCKED_REFERENCE_STATUS:
                    reference = row['reference']
                    artifacts.append({'state_id': row['id'], 'variant_id': row.get('variantId'), 'kind': 'blocked-reference', 'path': reference, 'sha256': digest(safe_path(root, reference))})
                    continue
                for kind, filename in [('reference', row['reference']), ('baseline', str((report_path.parent / row['baselineCopy']).relative_to(root))), ('candidate', str((report_path.parent / row['candidate']).relative_to(root))), ('diff', str((report_path.parent / row['diff']).relative_to(root)))]:
                    artifacts.append({'state_id': row['id'], 'variant_id': row.get('variantId'), 'kind': kind, 'path': filename, 'sha256': digest(safe_path(root, filename))})
            for filename in ['typescript-tests.log', 'production-build.log', 'controlled-build.log', 'vite.log']:
                file = evidence_path(report_path.parent, filename)
                artifacts.append({'kind': 'build-or-browser-log', 'path': str(file.relative_to(root)), 'sha256': digest(file)})
            details.update(behavior_status=behavior_status, visual_status=visual_status, behavior_checks=len(report['behavior']), visual_states=len(report['captures']), artifacts=artifacts,
                           capture_receipt=str(report_path.relative_to(root)), capture_receipt_sha256=digest(report_path), independent_comparison=str(recompute_path.relative_to(root)), independent_comparison_sha256=digest(recompute_path))
        except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
            errors.append('Product capture evidence inspection failed: ' + str(error))
    elif check_id in PRODUCT_RUNTIME_CHECKS:
        try:
            if runtime_input_error: raise ValueError(runtime_input_error)
            from runtime_stability_evidence import validate_run
            report_path=evidence_path(directory/'runtime','verification.json')
            report=json.loads(report_path.read_text())
            if report.get('execution_nonce')!=nonce or not int(started_wall*1000)<=report.get('executed_at_ms',0)<=int(completed_wall*1000):
                raise ValueError('Runtime report is not fresh/current launcher nonce')
            recomputed=validate_run(root,directory/'runtime',nonce,before)
            recomputed_path=directory/'runtime-recomputed.json'
            recomputed_path.write_text(json.dumps(recomputed,ensure_ascii=False,indent=2)+'\n')
            for key in ('status','errors','cases','source_tree_sha256','artifact_sha256','main_pid','fixture_substitution','product_packaged_native_game_tests_executed'):
                if report.get(key)!=recomputed.get(key): errors.append('Independent runtime recomputation differs: '+key)
            errors.extend(recomputed['errors'])
            from runtime_stability_evidence import CASE_IDS
            rows=recomputed.get('cases',[])
            if recomputed['status']!='passed': errors.append('Actual packaged stability scenarios did not all pass')
            if len(rows)!=len(CASE_IDS) or {r.get('id') for r in rows}!=set(CASE_IDS) or any(r.get('status')!='passed' or type(r.get('assertion_count')) is not int or r['assertion_count']<=0 for r in rows) or type(recomputed.get('assertion_count')) is not int or recomputed['assertion_count']<=0:
                errors.append('Actual runtime requires every concrete case and nonzero executed assertions')
            if recomputed.get('fixture_substitution') is not False or recomputed.get('product_packaged_native_game_tests_executed') is not True:
                errors.append('Runtime fixture/no-package substitution is forbidden')
            details.update(product_packaged_native_game_tests_executed=recomputed['product_packaged_native_game_tests_executed'],
                           fixture_substitution=False,artifact_sha256=recomputed.get('artifact_sha256'),
                           runtime_receipt=str(report_path.relative_to(root)),runtime_receipt_sha256=digest(report_path),
                           independent_runtime_recomputation=str(recomputed_path.relative_to(root)),independent_runtime_recomputation_sha256=digest(recomputed_path),
                           runtime_case_count=len(recomputed['cases']),assertion_count=recomputed['assertion_count'])
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append('Actual packaged runtime evidence inspection failed: '+str(error))
        if code: errors.append('Actual packaged runtime command failed with exit '+str(code))
    elif check_id == 'product-preview-visual-tests':
        from preview_visual_evidence import inspect as inspect_preview
        preview_errors, preview_details = inspect_preview(root, directory / 'preview', nonce, started_wall, completed_wall)
        errors.extend(preview_errors); details.update(preview_details)
        if code: errors.append('Actual preview visual command failed with exit ' + str(code))
    elif check_id == 'product-cp01-reading-tests':
        from cp01_component_evidence import inspect as inspect_cp01
        cp01_errors, cp01_details = inspect_cp01(root, directory / 'component', nonce, started_wall, completed_wall)
        errors.extend(cp01_errors); details.update(cp01_details)
        if code: errors.append('Actual CP01 component command failed with exit ' + str(code))
    elif check_id == 'product-settings-controls-tests':
        from settings_controls_evidence import inspect as inspect_settings_controls
        settings_errors, settings_details = inspect_settings_controls(root, directory / 'settings', nonce, started_wall, completed_wall)
        errors.extend(settings_errors); details.update(settings_details)
        if code: errors.append('Actual settings controls command failed with exit ' + str(code))
    elif check_id == WINDOW_TITLE_PROOF_CHECK:
        try:
            api_errors, api_details = validate_window_title_proof_report(root, directory / 'winapi', nonce)
            errors.extend(api_errors)
            details.update(api_details)
        except (OSError, ValueError, KeyError, TypeError) as error:
            errors.append('Actual WinAPI receipt inspection failed: ' + str(error))
        if code:
            errors.append('Actual maintained WinAPI command failed with exit ' + str(code))
    else:
        count, cargo_errors = validate_cargo_output(stdout + '\n' + stderr)
        errors.extend(cargo_errors)
        details['test_count'] = count
        if code:
            errors.append('Actual production Rust test command failed with exit ' + str(code))
    receipt = {'schema_version': 1, 'adapter_id': check_id, 'run_id': run_id, 'status': 'failed' if errors else 'passed',
               'errors': errors, 'source_tree_sha256': before, 'execution_nonce': nonce, 'command': command, 'exit_code': code,
               'elapsed_seconds': round(time.monotonic() - started, 3), 'log': str(log.relative_to(root)), 'log_sha256': digest(log),
               'product_component_tests_executed': bool(details.get('test_count', 0) or details.get('behavior_checks', 0)),
               'product_packaged_native_game_tests_executed': False, **details}
    receipt_path = directory / 'execution.json'
    receipt_path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    return {**receipt, 'receipt': str(receipt_path.relative_to(root)), 'receipt_sha256': digest(receipt_path)}


def main():
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='append', required=True,
                        choices=sorted(set(PRODUCT_CAPTURE_CHECKS) | set(PRODUCT_TEST_CHECKS) | set(PRODUCT_RUNTIME_CHECKS)))
    parser.add_argument('--output-root', default='.build/quality/product-execution')
    parser.add_argument('--runtime-inputs', help='Explicit frozen runtime inputs under repository')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = safe_path(root, args.output_root)
    results = []
    for check_id in args.check:
        try:
            result = run_product_check(root, check_id, output, args.runtime_inputs)
        except (OSError, ValueError, KeyError, TypeError) as error:
            result = {'adapter_id': check_id, 'status': 'failed', 'errors': [str(error)]}
        results.append(result)
        print(json.dumps({key: result.get(key) for key in ['adapter_id', 'status', 'errors', 'test_count', 'behavior_status', 'visual_status', 'receipt']}, ensure_ascii=False), flush=True)
    return int(any(result['status'] != 'passed' for result in results))


if __name__ == '__main__':
    raise SystemExit(main())
