"""Fresh real React component checks; no native, game or pixel-approval claim."""
from datetime import datetime, timezone
import hashlib
import json
import os
import re
from pathlib import Path
import struct
import subprocess
import time
import uuid
import zlib

from gate_core import digest, safe_path, source_files

CHECK_ID = 'product-reading-component-tests'
# RF07 obligations are independent of the producer's reported case inventory.
HISTORY_RANGE_RECOVERY_CHECKS = frozenset({
    'RBC-51-follow-expired-opens-one-new-range',
    'RBC-52-paused-expired-keeps-pages-and-anchor-until-latest',
    'RBC-53-expiry-recovery-failure-retains-rows-and-retries-latest',
    'RBC-54-late-expiry-old-filter-session-dataset-is-ignored',
    'RBC-55-pause-during-expiry-recovery-discards-new-range',
    'RBC-56-nonexpiry-and-wrong-stage-errors-never-reset-range',
    'RBC-57-invalid-cursor-requires-explicit-latest',
    'RBC-58-expiry-with-queued-navigation-preserves-priority',
    'RBC-59-expiry-recovery-single-flight',
    'RBC-60-older-expiry-keeps-old-range-until-latest',
})
# Frontend lifecycle obligations do not stand in for AppKit or game evidence.
WORDCARD_LIFECYCLE_CHECKS = frozenset({
    'RBC-61-wordcard-content-updates-and-owner-resize-preserve-frozen-object',
    'RBC-62-exact-native-closed-event-isolates-new-card',
    'RBC-63-escape-and-owner-outside-pointer-close-without-blur-close',
    'RBC-64-owner-scroll-closes-but-surface-and-unrelated-scroll-do-not',
    'RBC-65-owner-unmount-and-late-open-close-exact-card',
    'RBC-66-native-close-before-open-reply-and-frozen-new-card-isolation',
})
# Preview failures must terminate visibly without manufacturing native evidence.
PREVIEW_FAILURE_CHECKS = frozenset({
    'RBC-67-candidate-typed-start-failure-is-terminal-and-explicitly-retryable',
    'RBC-68-candidate-start-rejection-never-spins-or-retries-without-intent',
    'RBC-69-candidate-permission-edge-recovers-once-without-changing-hook',
    'RBC-70-session-typed-terminal-clears-frame-and-only-reattaches-rust-lease',
    'RBC-71-session-ipc-rejection-is-visible-without-client-start-or-stop',
    'RBC-72-candidate-transport-auto-retry-is-bounded-and-explicitly-reset',
    'RBC-73-candidate-target-change-and-cancel-isolate-late-start',
})
PREVIEW_PICKER_CHECKS = frozenset({
    'RBC-74-picker-requires-explicit-candidate-intent-and-shows-selected-frame',
    'RBC-75-picker-cancel-timeout-mismatch-busy-and-ipc-failure-stay-terminal',
    'RBC-76-picker-late-candidate-target-result-cannot-replace-next-preview',
    'RBC-77-session-picker-is-explicit-and-does-not-client-start-or-stop',
    'RBC-78-picker-late-session-result-only-detaches-old-session',
    'RBC-79-picker-candidate-cancel-releases-late-selected-lease',
    'RBC-80-real-tauri-picker-adapter-uses-only-explicit-narrow-select-ipc',
    'RBC-81-real-tauri-late-select-or-start-reply-cannot-replace-new-pull',
})
# B obligations are independent of both the runner and its source-case inventory.
# Component/adapter evidence is never native collector or signed-package evidence.
PREVIEW_SUBSCRIPTION_CHECKS = frozenset({
    'RBC-82-typed-transient-binding-rejection-retains-decoded-frame-and-subscription',
    'RBC-83-initial-binding-rejection-never-reuses-an-old-game-frame',
    'RBC-84-typed-error-stage-and-recoverable-are-not-message-heuristics',
    'RBC-85-successful-binding-read-clears-only-own-transient-notice',
    'RBC-86-authoritative-unbound-and-stale-invalidate-old-frame-immediately',
    'RBC-87-late-old-session-binding-errors-and-frames-cannot-enter-new-session',
    'RBC-88-same-target-new-collector-incarnation-rejects-old-lease-generation',
    'RBC-89-real-adapter-same-context-two-subscribers-both-receive-later-frames',
    'RBC-90-unique-unsubscribe-is-idempotent-and-never-cancels-new-subscriber',
    'RBC-91-single-flight-pull-and-late-finally-do-not-delete-new-context',
    'RBC-92-subscription-transport-failure-is-not-authoritative-target-loss-or-live',
    'RBC-93-paused-retained-frame-survives-metadata-errors-and-own-remount',
    'RBC-94-permission-revocation-selection-expiry-and-target-loss-never-stay-live',
    'RBC-95-listener-exception-and-unresolved-consumer-do-not-own-producer',
    'RBC-96-real-component-navigation-and-old-cleanup-preserve-another-subscriber',
    'RBC-97-natural-binding-metadata-and-frame-poll-overlap-preserves-context',
})
# Native leases begin unresolved; first real frame resolution is independently required.
PREVIEW_FIRST_FRAME_CHECKS = frozenset({
    'RBC-98-real-adapter-unresolved-target-first-frame-binds-once',
    'RBC-99-real-component-unresolved-candidate-and-session-decode-first-frame',
    'RBC-100-no-successful-frame-timeout-never-claims-an-expired-image',
})
# Independently registered obligations. Never derive the required set from a receipt.
REQUIRED_CHECKS = frozenset({
    'RBC-01-custom-select-not-visible-native',
    'RBC-02-active-single-gap-no-row-shrink',
    'RBC-03-cancel-outside-and-long-confirm',
    'RBC-04-disabled-empty-loading-error-recovery',
    'RBC-05-eight-independent-annotation-combinations',
    'RBC-06-invalid-span-and-unknown-reading-preserve-source',
    'RBC-07-select-200-percent-and-popup-scroll-only',
    'RBC-08-lexical-failures-and-stale-identity',
    'RBC-09-hud-common-controls-front-at-both-widths',
    'RBC-10-hud-native-drag-intent-and-local-scroll-menu',
    'RBC-11-preview-frame-lease-pause-and-late-session-isolation',
    'RBC-12-session-pagination-full-search-fixed-range-and-export',
    'RBC-13-hook-pages-failure-retry-late-search-and-hook-only-new-count',
    'RBC-14-preview-permission-does-not-change-hook-session',
    'RBC-15-hud-natural-extent-grows-shrinks-and-deduplicates',
    'RBC-16-full-session-scroll-owner-reaches-preview-and-history',
    'RBC-17-compact-preference-keeps-base-fonts-with-one-effective-scale',
    'RBC-18-scoped-preference-mask-absent-head-cas-and-inheritance',
    'RBC-19-scope-read-failure-preserves-dirty-draft-until-explicit-discard',
    'RBC-20-native-capability-failure-single-flight-and-late-generation',
    'RBC-21-save-in-flight-keeps-newer-draft-and-advances-cas',
    'RBC-22-final-page-refresh-retry-keeps-fixed-range-and-scroll',
    'RBC-23-bound-window-activation-result-and-unbind-preserves-capture',
    'RBC-24-activation-wrong-target-and-late-session-cannot-claim-success',
    'RBC-25-hud-tooltips-use-only-native-effective-shortcuts',
    'RBC-26-late-unbind-success-and-failure-do-not-clear-next-session',
    'RBC-27-proof-busy-keeps-last-bound-preview-and-real-stale-clears-it',
    'RBC-28-same-session-window-rebind-isolates-late-unbind',
    'RBC-29-new-hook-session-without-body-has-no-previous-reading-actions',
    'RBC-30-late-previous-session-history-cannot-become-current-reading',
    'RBC-31-current-session-reading-arrival-keeps-action-object-identity',
    'RBC-32-stopped-session-last-reading-keeps-matching-actions',
    'RBC-33-body-late-translation-refreshes-with-zero-newer-and-frozen-revision',
    'RBC-34-loaded-body-pages-refresh-content-and-preserve-cursor-and-anchor',
    'RBC-35-paused-body-refresh-keeps-high-watermark-while-new-count-arrives',
    'RBC-36-body-stale-scope-replies-and-overlapping-page-reads-are-isolated',
    'RBC-37-body-refresh-failure-keeps-all-pages-and-retry-reconciles-content',
    'RBC-38-body-retranslation-and-correction-refresh-only-matching-history-object',
    'RBC-39-cover-decodes-real-bytes-and-reads-once-per-reference',
    'RBC-40-corrupt-missing-and-wrong-owner-covers-show-fallback',
    'RBC-41-running-only-same-panel-one-confirm-and-explicit-stream',
    'RBC-42-candidate-preview-starts-automatically-and-cancel-releases-only-candidate',
    'RBC-43-session-preview-persists-across-tabs-and-navigation-without-stop',
    'RBC-44-diagnostic-default-and-full-log-query-search-paging-export',
    'RBC-45-preview-permission-recovery-is-automatic-and-hook-independent',
    'RBC-46-real-tauri-transport-failure-and-candidate-recovery-do-not-leak',
    'RBC-47-association-freezes-the-confirmed-target-until-receipt',
    'RBC-48-initial-scan-pending-strict-replay-and-real-single-flight',
    'RBC-49-existing-library-association-is-visible-and-target-scoped',
    'RBC-50-new-record-typed-save-failure-and-retry-do-not-misassociate',
}) | HISTORY_RANGE_RECOVERY_CHECKS | WORDCARD_LIFECYCLE_CHECKS | PREVIEW_FAILURE_CHECKS | PREVIEW_PICKER_CHECKS | PREVIEW_SUBSCRIPTION_CHECKS | PREVIEW_FIRST_FRAME_CHECKS



CURRENT_SESSION_SCENARIOS = (
    '01-new-session-waiting', '02-old-session-late',
    '03-new-session-body', '04-disconnected-last-sentence',
)
FIXTURE_SOURCE_PATHS = {
    'snapshotQuerySourceSha256': 'crates/app-infra/src/storage/queries/mod.rs',
    'producerSourceSha256': 'crates/app-infra/tests/application_runtime.rs',
}
READING_HISTORY_SOURCE_PATHS = frozenset({
    'crates/app-core/src/application/runtime.rs',
    'crates/app-core/src/contracts/desktop.rs',
    'crates/app-core/src/domain/translation_status.rs',
    'crates/app-infra/src/storage/database.rs',
    'crates/app-infra/src/storage/queries/mod.rs',
    'crates/app-infra/src/storage/queries/tasks.rs',
    'crates/app-infra/src/storage/queries/translation_status.rs',
    'crates/app-infra/tests/application_runtime.rs',
})
FIXTURE_COMMAND = ['cargo', 'test', '--locked', '--offline', '-p', 'app-infra',
                   '--test', 'application_runtime',
                   'exports_current_session_component_snapshots', '--', '--exact']


def fixture_sources(root):
    return {key: digest(safe_path(root, path)) for key, path in FIXTURE_SOURCE_PATHS.items()}


def history_fixture_sources(root):
    return {path: digest(safe_path(root, path)) for path in sorted(READING_HISTORY_SOURCE_PATHS)}


def _unique_json_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError('Duplicate key in actual reading history/status fixture')
        value[key] = item
    return value


def _inspect_history_sidecar(directory, nonce, manifest, expected_sources, not_before_ns):
    if not isinstance(expected_sources, dict) or set(expected_sources) != READING_HISTORY_SOURCE_PATHS or any(not isinstance(value, str) or not re.fullmatch(r'[a-f0-9]{64}', value) for value in expected_sources.values()):
        raise ValueError('Actual reading history/status source identities are incomplete')
    entry = manifest.get('readingStatusHistory')
    if not isinstance(entry, dict) or set(entry) != {'path', 'sha256'} or entry.get('path') != 'reading-status-history.json' or not isinstance(entry.get('sha256'), str) or not re.fullmatch(r'[a-f0-9]{64}', entry['sha256']):
        raise ValueError('Actual reading history/status sidecar declaration is missing or unsupported')
    file = safe_path(directory, entry['path'])
    if not file.is_file() or not 0 < file.stat().st_size <= 8 * 1024 * 1024:
        raise ValueError('Actual reading history/status sidecar bytes do not match manifest')
    if not_before_ns is not None and file.stat().st_mtime_ns < not_before_ns:
        raise ValueError('Actual reading history/status sidecar predates this invocation')
    data = file.read_bytes()
    if not 0 < len(data) <= 8 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != entry['sha256']:
        raise ValueError('Actual reading history/status sidecar bytes do not match manifest')
    payload = json.loads(data, object_pairs_hook=_unique_json_object)
    if not isinstance(payload, dict) or type(payload.get('schemaVersion')) is not int or payload['schemaVersion'] != 1 or payload.get('kind') != 'actual_sqlite_reading_unit_projection_history' or payload.get('nonce') != nonce or payload.get('sourceSha256') != expected_sources:
        raise ValueError('Actual reading history/status sidecar identity mismatch')
    return {'path': entry['path'], 'sha256': entry['sha256'], 'sourceSha256': dict(expected_sources)}


def inspect_current_session_fixtures(directory, nonce, expected_sources, not_before_ns=None, *, expected_history_sources=None):
    """Bind the four actual Rust/SQLite DTOs; never construct substitute snapshots."""
    if not directory.is_dir() or directory.is_symlink():
        raise ValueError('Actual Rust fixture directory is missing or a symlink')
    if set(expected_sources) != set(FIXTURE_SOURCE_PATHS) or any(not isinstance(value, str) or not re.fullmatch(r'[a-f0-9]{64}', value) for value in expected_sources.values()):
        raise ValueError('Actual Rust fixture producer/query source identities are incomplete')
    manifest_path = safe_path(directory, 'manifest.json')
    if not manifest_path.is_file() or manifest_path.stat().st_size > 256 * 1024:
        raise ValueError('Missing or oversized actual Rust fixture manifest')
    if not_before_ns is not None and manifest_path.stat().st_mtime_ns < not_before_ns:
        raise ValueError('Actual Rust fixture manifest predates this invocation')
    manifest = json.loads(manifest_path.read_text())
    if not isinstance(manifest, dict) or type(manifest.get('schemaVersion')) is not int or manifest['schemaVersion'] != 1:
        raise ValueError('Invalid actual Rust fixture manifest schema')
    for key, value in {
        'kind': 'actual_sqlite_runtime_projection_fixtures',
        'producerTarget': 'application_runtime',
        'producer': 'exports_current_session_component_snapshots',
        'nonce': nonce,
        'notLiveGameEvidence': True,
        **expected_sources,
    }.items():
        if manifest.get(key) != value or type(manifest.get(key)) is not type(value):
            raise ValueError('Actual Rust fixture manifest identity mismatch: ' + key)
    if not isinstance(nonce, str) or not re.fullmatch(r'[a-f0-9]{32}', nonce):
        raise ValueError('Actual Rust fixtures require this wrapper UUID hex nonce')
    entries = manifest.get('entries')
    if not isinstance(entries, list) or any(not isinstance(row, dict) for row in entries):
        raise ValueError('Actual Rust fixture entries must be structured rows')
    scenarios = [row.get('scenario') for row in entries]
    if len(entries) != 4 or any(not isinstance(value, str) for value in scenarios) or set(scenarios) != set(CURRENT_SESSION_SCENARIOS):
        raise ValueError('Every exact current-session scenario must occur once')
    expected_files = {'manifest.json'}
    payloads, files = {}, []
    for entry in entries:
        if set(entry) != {'scenario', 'path', 'sha256', 'sessionId', 'current'}:
            raise ValueError('Actual Rust fixture entry is incomplete or unsupported')
        scenario, name = entry['scenario'], entry.get('path')
        if not isinstance(name, str) or name != scenario + '.json' or Path(name).name != name or '/' in name or '\\' in name:
            raise ValueError('Unsafe or mismatched actual Rust fixture basename')
        file = safe_path(directory, name)
        expected_files.add(name)
        if not file.is_file() or file.stat().st_size > 8 * 1024 * 1024 or digest(file) != entry.get('sha256'):
            raise ValueError('Actual Rust fixture bytes do not match manifest: ' + scenario)
        if not_before_ns is not None and file.stat().st_mtime_ns < not_before_ns:
            raise ValueError('Actual Rust fixture payload predates this invocation')
        snapshot = json.loads(file.read_text())
        session = snapshot.get('session') if isinstance(snapshot, dict) else None
        if not isinstance(session, dict) or session.get('sourceKind') != 'hook' or not isinstance(session.get('sessionId'), str) or not session['sessionId'] or entry.get('sessionId') != session['sessionId']:
            raise ValueError('Actual Rust fixture has no matching hook session: ' + scenario)
        current = snapshot.get('current')
        if scenario in CURRENT_SESSION_SCENARIOS[:2]:
            if current is not None or entry.get('current') is not None or session.get('state') not in ('opening', 'active'):
                raise ValueError('Waiting/late callback snapshot must not expose the previous sentence')
        else:
            if not isinstance(current, dict) or not isinstance(current.get('unitId'), str) or not current['unitId'] or type(current.get('textRevision')) is not int or current['textRevision'] <= 0 or current.get('sessionId') != session['sessionId'] or not isinstance(current.get('sourceText'), str) or not current['sourceText']:
                raise ValueError('Current sentence identity is invalid: ' + scenario)
            projected = {key: current[key] for key in ('unitId', 'textRevision', 'sessionId', 'sourceText')}
            if entry.get('current') != projected:
                raise ValueError('Actual Rust manifest current identity differs from its payload')
            expected_state = 'active' if scenario == CURRENT_SESSION_SCENARIOS[2] else 'stopped'
            if session.get('state') != expected_state:
                raise ValueError('Actual Rust fixture session state differs from scenario')
        if snapshot.get('credentials') != [] or snapshot.get('credentialPools') != []:
            raise ValueError('Controlled component fixtures must not contain credentials')
        payloads[scenario] = snapshot
        files.append({'scenario': scenario, 'path': name, 'sha256': digest(file)})
    history_evidence = None
    if expected_history_sources is not None:
        history_evidence = _inspect_history_sidecar(directory, nonce, manifest, expected_history_sources, not_before_ns)
        if any(expected_history_sources[path] != expected_sources[key] for key, path in FIXTURE_SOURCE_PATHS.items()):
            raise ValueError('Actual reading history/status sources differ from the original fixture sources')
        expected_files.add(history_evidence['path'])
    elif 'readingStatusHistory' in manifest:
        raise ValueError('Actual reading history/status sidecar requires all current source identities')
    if {entry.name for entry in directory.iterdir()} != expected_files:
        raise ValueError('Actual Rust fixture directory contains unregistered output')
    sessions = {value['session']['sessionId'] for value in payloads.values()}
    if len(sessions) != 1:
        raise ValueError('Actual Rust scenario sequence changed the new session identity')
    for scenario in CURRENT_SESSION_SCENARIOS[:2]:
        history_page = payloads[scenario].get('history')
        history = history_page.get('units') if isinstance(history_page, dict) else None
        if not isinstance(history, list) or not history or any(not isinstance(unit, dict) or unit.get('sessionId') in sessions for unit in history):
            raise ValueError('Waiting/late fixture must retain old history without making it current')
    if payloads[CURRENT_SESSION_SCENARIOS[2]]['current'] != payloads[CURRENT_SESSION_SCENARIOS[3]]['current']:
        # sourceSummary state/timestamps legitimately change after disconnect.
        current_keys = ('unitId', 'textRevision', 'sessionId', 'sourceText', 'translations')
        if any(payloads[CURRENT_SESSION_SCENARIOS[2]]['current'].get(key) != payloads[CURRENT_SESSION_SCENARIOS[3]]['current'].get(key) for key in current_keys):
            raise ValueError('Disconnect changed the fixed last sentence/action identity')
    evidence = {
        'fixtureManifestSha256': digest(manifest_path), 'nonce': nonce,
        'producerTarget': manifest['producerTarget'], 'producer': manifest['producer'],
        **expected_sources, 'files': sorted(files, key=lambda row: row['scenario']),
    }
    if history_evidence is not None:
        evidence['readingStatusHistory'] = history_evidence
    return evidence


def validate_fixture_command(stdout, exit_code):
    if exit_code != 0 or len(re.findall(r'^test exports_current_session_component_snapshots \.\.\. ok$', stdout, re.MULTILINE)) != 1 or not re.search(r'^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; \d+ filtered out;', stdout, re.MULTILINE):
        raise ValueError('Actual Rust fixture producer must execute exactly one passing, unskipped test')


def component_sources(root):
    """Match the producer's complete source/test byte inventory, independently."""
    root = root.resolve()
    result = {}
    for prefix in ('apps/desktop/src', 'apps/desktop/tests'):
        directory = safe_path(root, prefix)
        if not directory.is_dir():
            raise ValueError('Missing component source root: ' + prefix)
        for file in directory.rglob('*'):
            if file.is_symlink():
                raise ValueError('Component sources may not contain symlinks')
            if file.is_file():
                result[file.relative_to(root).as_posix()] = digest(file)
    if not result:
        raise ValueError('Component source inventory is empty')
    return dict(sorted(result.items()))


def tree_identity(root):
    return hashlib.sha256(json.dumps(source_files(root), sort_keys=True).encode()).hexdigest()


def inspect_png(file):
    """Inspect real PNG bytes and chunk integrity without executing the producer."""
    data = file.read_bytes()
    if len(data) > 64 * 1024 * 1024:
        raise ValueError('Capture exceeds the PNG inspection byte budget')
    if not data.startswith(b'\x89PNG\r\n\x1a\n'):
        raise ValueError('Capture is not PNG bytes')
    offset, idat, header, ended = 8, [], None, False
    while offset < len(data):
        if offset + 12 > len(data):
            raise ValueError('Truncated PNG chunk')
        size = struct.unpack('>I', data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        end = offset + 12 + size
        if end > len(data):
            raise ValueError('Truncated PNG payload')
        payload = data[offset + 8:offset + 8 + size]
        crc = struct.unpack('>I', data[end - 4:end])[0]
        if zlib.crc32(kind + payload) & 0xffffffff != crc:
            raise ValueError('Invalid PNG chunk CRC')
        if header is None:
            if kind != b'IHDR' or size != 13:
                raise ValueError('Missing PNG IHDR')
            header = struct.unpack('>IIBBBBB', payload)
            if not header[0] or not header[1]:
                raise ValueError('Empty PNG dimensions')
            if header[0] * header[1] > 16 * 1024 * 1024:
                raise ValueError('Capture exceeds the PNG inspection pixel budget')
        elif kind == b'IHDR':
            raise ValueError('Duplicate PNG IHDR')
        if kind == b'IDAT':
            idat.append(payload)
        if kind == b'IEND':
            if size or end != len(data):
                raise ValueError('Invalid PNG end/trailing bytes')
            ended = True
            break
        offset = end
    if not ended or not idat:
        raise ValueError('Incomplete PNG image')
    # CRC and successful inflate alone do not prove valid pixel scanlines.
    # Reuse the already locked decoder used by the visual comparator.
    decoder = Path(__file__).resolve().parents[2] / 'node_modules/pngjs'
    code = ('const fs=require("node:fs"),{PNG}=require(process.argv[1]);'
            'const p=PNG.sync.read(fs.readFileSync(process.argv[2]),{checkCRC:true});'
            'if(p.data.length!==p.width*p.height*4)throw Error("Invalid decoded pixels");')
    decoded = subprocess.run(['node', '-e', code, str(decoder), str(file)],
                             text=True, capture_output=True, timeout=15)
    if decoded.returncode:
        raise ValueError('Capture PNG pixel decoding failed: ' + decoded.stderr[:500])


def validate_report(directory, report, nonce, expected_sources, exit_code, expected_fixtures):
    """Fail closed on missing/old/zero-assertion/unsafe or inflated evidence."""
    errors = []
    if exit_code != 0:
        errors.append('Actual component command did not exit zero')
    if type(report.get('schemaVersion')) is not int or report.get('schemaVersion') != 1 or report.get('layer') != 'component':
        errors.append('Invalid component receipt schema/layer')
    if report.get('nonce') != nonce or not nonce:
        errors.append('Component receipt nonce is not this execution')
    if report.get('sourceSha256') != expected_sources or not expected_sources:
        errors.append('Component receipt source inventory differs from current full source/test bytes')
    if report.get('status') != 'passed' or report.get('errors') != []:
        errors.append('Component producer reported failure/errors')
    if report.get('nativeOrGameVerified') is not False or report.get('visualPixelAcceptance') is not False:
        errors.append('Browser components cannot claim native/game or visual pixel acceptance')
    actual_fixtures = report.get('currentSessionFixtures')
    fixture_matches = isinstance(actual_fixtures, dict) and bool(expected_fixtures)
    if fixture_matches:
        fixture_matches = all(actual_fixtures.get(key) == value for key, value in expected_fixtures.items() if key not in {'files', 'readingStatusHistory'})
        expected_history = expected_fixtures.get('readingStatusHistory')
        if expected_history is not None:
            actual_history = actual_fixtures.get('readingStatusHistory')
            fixture_matches = fixture_matches and isinstance(actual_history, dict)
            if isinstance(actual_history, dict):
                fixture_matches = fixture_matches and set(actual_history) in ({'path', 'sha256', 'sourceSha256'}, {'path', 'sha256', 'sourceSha256', 'fileRootRelative'})
                fixture_matches = fixture_matches and all(actual_history.get(key) == value for key, value in expected_history.items())
                fixture_matches = fixture_matches and ('fileRootRelative' not in actual_history or isinstance(actual_history['fileRootRelative'], str))
        elif 'readingStatusHistory' in actual_fixtures:
            fixture_matches = False
        actual_files = actual_fixtures.get('files')
        fixture_matches = fixture_matches and isinstance(actual_files, list) and len(actual_files) == 4 and all(isinstance(row, dict) for row in actual_files)
        if fixture_matches:
            normalized = [{key: row.get(key) for key in ('scenario', 'path', 'sha256')} for row in actual_files]
            fixture_matches = sorted(normalized, key=lambda row: str(row['scenario'])) == expected_fixtures['files']
    if not fixture_matches:
        errors.append('Component receipt is not bound to this actual Rust fixture manifest and payloads')
    rows = report.get('checks')
    if not isinstance(rows, list) or any(not isinstance(row, dict) for row in rows):
        errors.append('Component checks must be structured rows')
        rows = []
    ids = [row.get('id') for row in rows]
    if len(rows) != len(REQUIRED_CHECKS) or any(not isinstance(value, str) for value in ids) or set(value for value in ids if isinstance(value, str)) != REQUIRED_CHECKS or len(set(value for value in ids if isinstance(value, str))) != len(rows):
        errors.append('Every exact registered RBC check must execute once, without extras or duplicates')
    total = 0
    for row in rows:
        count = row.get('assertionCount')
        if row.get('status') != 'passed' or type(count) is not int or count <= 0 or any(row.get(key) not in (None, False, 0) for key in ('skipped', 'skip', 'todo', 'cancelled')):
            errors.append('Each RBC check must pass actual positive assertions without skip: ' + str(row.get('id')))
        if type(count) is int and count > 0:
            total += count
    if type(report.get('assertionCount')) is not int or report['assertionCount'] < total or total <= 0:
        errors.append('Total actual assertion count is missing or smaller than per-check counts')
    captures = report.get('captures')
    if not isinstance(captures, list) or not captures:
        errors.append('At least one fresh actual component PNG is required')
        captures = []
    seen = set()
    for row in captures:
        try:
            name = row.get('file') if isinstance(row, dict) else None
            if not isinstance(name, str) or not name or '\\' in name or '/' in name or Path(name).name != name or not name.endswith('.png') or name in seen:
                raise ValueError('Unsafe, duplicate or non-PNG capture basename')
            seen.add(name)
            file = safe_path(directory, name)
            if not file.is_file() or digest(file) != row.get('sha256'):
                raise ValueError('Capture bytes do not match this report hash')
            inspect_png(file)
        except (OSError, ValueError, TypeError, zlib.error, struct.error, subprocess.TimeoutExpired) as error:
            errors.append('Invalid component capture: ' + str(error))
    return errors


def run_reading_components(root, env=None):
    root = root.resolve()
    run_id, nonce = uuid.uuid4().hex, uuid.uuid4().hex
    directory = safe_path(root, '.build/quality/reading-components/' + run_id)
    directory.mkdir(parents=True, exist_ok=False)
    capture = directory / 'capture'  # Both actual producers create fresh child directories.
    fixtures = directory / 'current-session-fixtures'
    fixture_manifest = fixtures / 'manifest.json'
    command = ['node', 'apps/desktop/tests/visual/verify-reviewed-reading.mjs',
               '--output', str(capture), '--nonce', nonce,
               '--current-session-fixtures', str(fixture_manifest)]
    before, sources = tree_identity(root), component_sources(root)
    started, started_at, started_ns = time.monotonic(), datetime.now(timezone.utc).isoformat(), time.time_ns()
    errors, fixture_evidence, fixture_code = [], None, -1
    fixture_stdout, fixture_stderr, stdout, stderr, code = '', '', '', '', -1
    expected_fixture_sources, expected_history_sources = {}, {}
    fixture_log = directory / 'fixture-execution.log'
    child_env = {**os.environ, **(env or {})}
    child_env.update(LUNA_CURRENT_SESSION_FIXTURE_DIR=str(fixtures),
                     LUNA_CURRENT_SESSION_FIXTURE_NONCE=nonce)
    try:
        expected_fixture_sources = fixture_sources(root)
        expected_history_sources = history_fixture_sources(root)
        if fixtures.exists() or capture.exists():
            raise ValueError('Actual component/fixture producers require fresh output directories')
        process = subprocess.run(FIXTURE_COMMAND, cwd=root, env=child_env,
                                 capture_output=True, text=True, timeout=600)
        fixture_stdout, fixture_stderr, fixture_code = process.stdout, process.stderr, process.returncode
        validate_fixture_command(fixture_stdout, fixture_code)
        safe_path(root, fixtures.relative_to(root).as_posix())
        fixture_evidence = inspect_current_session_fixtures(fixtures, nonce, expected_fixture_sources, started_ns, expected_history_sources=expected_history_sources)
        if tree_identity(root) != before or fixture_sources(root) != expected_fixture_sources or history_fixture_sources(root) != expected_history_sources:
            raise ValueError('Repository source changed during actual Rust fixture generation')
    except (OSError, ValueError, TypeError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        errors.append('Actual Rust fixture generation failed: ' + str(error))
    fixture_log.write_text(fixture_stdout + '\n' + fixture_stderr)
    if not errors:
        try:
            process = subprocess.run(command, cwd=root, env=child_env, capture_output=True, text=True, timeout=600)
            stdout, stderr, code = process.stdout, process.stderr, process.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            stdout, stderr, code = '', str(error), -1
    log = directory / 'execution.log'
    log.write_text(stdout + '\n' + stderr)
    after = tree_identity(root)
    if before != after:
        errors.append('Full repository source identity drifted during component execution')
    report, report_path, report_sha256 = {}, capture / 'verification.json', None
    if fixture_evidence is not None and not errors:
        try:
            safe_path(root, fixtures.relative_to(root).as_posix())
            if inspect_current_session_fixtures(fixtures, nonce, expected_fixture_sources, started_ns, expected_history_sources=expected_history_sources) != fixture_evidence or fixture_sources(root) != expected_fixture_sources or history_fixture_sources(root) != expected_history_sources:
                raise ValueError('Actual Rust fixture bytes/source changed while consumed by Node')
            safe_path(root, report_path.relative_to(root).as_posix())
            report = json.loads(report_path.read_text())
            if not isinstance(report, dict):
                raise ValueError('Component report is not an object')
            report_sha256 = digest(report_path)
            errors.extend(validate_report(capture, report, nonce, sources, code, fixture_evidence))
            if component_sources(root) != sources:
                errors.append('Component source/test bytes changed during execution')
        except (OSError, ValueError, TypeError, json.JSONDecodeError) as error:
            errors.append('Actual component receipt inspection failed: ' + str(error))
    receipt = dict(schema_version=1, adapter_id=CHECK_ID, run_id=run_id, nonce=nonce,
                   status='failed' if errors else 'passed', errors=errors, command=command,
                   exit_code=code, test_count=len(report['checks']) if isinstance(report.get('checks'), list) else 0, assertion_count=report.get('assertionCount', 0),
                   fixture_command=FIXTURE_COMMAND, fixture_exit_code=fixture_code,
                   fixture_log=fixture_log.relative_to(root).as_posix(),
                   current_session_fixtures=fixture_evidence,
                   fixture_manifest=fixture_manifest.relative_to(root).as_posix(),
                   actual_sqlite_runtime_fixture_test_executed=fixture_evidence is not None and fixture_code == 0,
                   started_at=started_at, elapsed_seconds=round(time.monotonic() - started, 3),
                   source_tree_sha256_before=before, source_tree_sha256_after=after,
                   log=log.relative_to(root).as_posix(),
                   component_receipt=report_path.relative_to(root).as_posix(),
                   component_receipt_sha256=report_sha256,
                   product_component_tests_executed=bool(report) and code == 0 and not errors,
                   product_packaged_native_game_tests_executed=False,
                   nativeOrGameVerified=False, visualPixelAcceptance=False,
                   scope='actual Rust/SQLite projection fixtures and real React with controlled ports; no packaged/native/game or pixel acceptance')
    (directory / 'execution.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    return receipt
