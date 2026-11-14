"""Guard-only fixtures exercise rejection; never a production execution adapter."""
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zlib

import reading_components as gate
real_subprocess_run = subprocess.run

# Independent contract list, not a copy of values returned by the adapter.
HISTORY_SOURCE_PATHS = frozenset({
    'crates/app-core/src/application/runtime.rs',
    'crates/app-core/src/contracts/desktop.rs',
    'crates/app-core/src/domain/translation_status.rs',
    'crates/app-infra/src/storage/database.rs',
    'crates/app-infra/src/storage/queries/mod.rs',
    'crates/app-infra/src/storage/queries/tasks.rs',
    'crates/app-infra/src/storage/queries/translation_status.rs',
    'crates/app-infra/tests/application_runtime.rs',
})


def png_bytes():
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\x00\x00\x00\x00\xff')) + chunk(b'IEND', b'')


def guard_fixture_outputs(directory, nonce, sources, history_sources=None):
    """Only rejection-test data; production always invokes the fixed Cargo test."""
    directory.mkdir(parents=True, exist_ok=False)
    entries = []
    for index, scenario in enumerate(gate.CURRENT_SESSION_SCENARIOS):
        current = None if index < 2 else dict(unitId='guard-unit', textRevision=1,
                                            sessionId='guard-next', sourceText='今の台詞。',
                                            translations=[{'text': '当前对白。'}])
        payload = dict(session=dict(sessionId='guard-next', sourceKind='hook',
                                    state=('opening', 'opening', 'active', 'stopped')[index]),
                       current=current, history={'units': [{'sessionId': 'guard-old'}]},
                       credentials=[], credentialPools=[])
        name = scenario + '.json'
        file = directory / name
        file.write_text(json.dumps(payload))
        projected = None if current is None else {key: current[key] for key in ('unitId', 'textRevision', 'sessionId', 'sourceText')}
        entries.append(dict(scenario=scenario, path=name, sha256=gate.digest(file),
                            sessionId='guard-next', current=projected))
    manifest = dict(schemaVersion=1, kind='actual_sqlite_runtime_projection_fixtures',
                    producerTarget='application_runtime',
                    producer='exports_current_session_component_snapshots',
                    nonce=nonce, notLiveGameEvidence=True, entries=entries, **sources)
    if history_sources is not None:
        sidecar = directory / 'reading-status-history.json'
        # Guard-only metadata: no fabricated current-state/page business DTOs.
        sidecar.write_text(json.dumps(dict(schemaVersion=1,
                                         kind='actual_sqlite_reading_unit_projection_history',
                                         nonce=nonce, sourceSha256=history_sources)))
        manifest['readingStatusHistory'] = dict(path=sidecar.name, sha256=gate.digest(sidecar))
    (directory / 'manifest.json').write_text(json.dumps(manifest))
    if history_sources is None:
        return gate.inspect_current_session_fixtures(directory, nonce, sources)
    return gate.inspect_current_session_fixtures(directory, nonce, sources,
                                                expected_history_sources=history_sources)


class ReadingComponentGuards(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for name in ('apps/desktop/src/control.tsx', 'apps/desktop/tests/control.mjs'):
            file = self.root / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('// guard fixture, not runtime product evidence\n')
        for name in set(gate.FIXTURE_SOURCE_PATHS.values()) | HISTORY_SOURCE_PATHS:
            file = self.root / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('// guard-only Rust source identity fixture\n')
        self.nonce = 'a' * 32
        self.fixture_sources = gate.fixture_sources(self.root)
        self.fixtures = self.root / '.build/guard-fixtures'
        self.fixture_evidence = guard_fixture_outputs(self.fixtures, self.nonce, self.fixture_sources)
        self.capture = self.root / '.build/capture'
        self.capture.mkdir(parents=True)
        self.image = self.capture / 'guard-fixture.png'
        self.image.write_bytes(png_bytes())
        self.sources = gate.component_sources(self.root)
        self.report = dict(schemaVersion=1, nonce=self.nonce, layer='component',
                           currentSessionFixtures=self.fixture_evidence,
                           status='passed', errors=[], nativeOrGameVerified=False,
                           visualPixelAcceptance=False, assertionCount=2 * len(gate.REQUIRED_CHECKS),
                           sourceSha256=self.sources,
                           checks=[dict(id=identity, status='passed', assertionCount=2) for identity in sorted(gate.REQUIRED_CHECKS)],
                           captures=[dict(file=self.image.name, sha256=hashlib.sha256(self.image.read_bytes()).hexdigest())])

    def errors(self, report=None, code=0):
        return gate.validate_report(self.capture, self.report if report is None else report, self.nonce, self.sources, code, self.fixture_evidence)

    def test_complete_guard_fixture_is_valid_component_only(self):
        legacy = gate.REQUIRED_CHECKS - gate.PREVIEW_SUBSCRIPTION_CHECKS - gate.PREVIEW_FIRST_FRAME_CHECKS
        self.assertEqual(len([identity for identity in gate.REQUIRED_CHECKS if int(identity.split('-')[1]) <= 50]), 50)
        self.assertEqual(len(legacy - gate.WORDCARD_LIFECYCLE_CHECKS - gate.PREVIEW_FAILURE_CHECKS - gate.PREVIEW_PICKER_CHECKS), 60)
        self.assertEqual(len(legacy - gate.PREVIEW_FAILURE_CHECKS - gate.PREVIEW_PICKER_CHECKS), 66)
        self.assertEqual(len(legacy - gate.PREVIEW_PICKER_CHECKS), 73)
        self.assertEqual(len(legacy), 81)
        self.assertEqual(len(gate.REQUIRED_CHECKS), 100)
        self.assertEqual(self.errors(), [])

    def test_previous_thirty_eight_checks_do_not_satisfy_repaired_product_gaps(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 38]
        self.assertEqual(len(old['checks']), 38)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_new_cover_binding_preview_or_diagnostic_obligation_is_required(self):
        added = sorted(identity for identity in gate.REQUIRED_CHECKS if 39 <= int(identity.split('-')[1]) <= 60)
        self.assertEqual(len(added), 22)
        for identity in added:
            with self.subTest(identity=identity):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))

    def test_rf07_history_recovery_has_exact_independent_required_obligations(self):
        expected = {
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
        }
        self.assertEqual(gate.HISTORY_RANGE_RECOVERY_CHECKS, expected)
        self.assertEqual({identity for identity in gate.REQUIRED_CHECKS if 51 <= int(identity.split('-')[1]) <= 60}, expected)
        self.assertEqual(len(expected), 10)

    def test_wordcard_lifecycle_has_exact_independent_required_obligations(self):
        expected = {
            'RBC-61-wordcard-content-updates-and-owner-resize-preserve-frozen-object',
            'RBC-62-exact-native-closed-event-isolates-new-card',
            'RBC-63-escape-and-owner-outside-pointer-close-without-blur-close',
            'RBC-64-owner-scroll-closes-but-surface-and-unrelated-scroll-do-not',
            'RBC-65-owner-unmount-and-late-open-close-exact-card',
            'RBC-66-native-close-before-open-reply-and-frozen-new-card-isolation',
        }
        self.assertEqual(gate.WORDCARD_LIFECYCLE_CHECKS, expected)
        self.assertEqual({identity for identity in gate.REQUIRED_CHECKS if 61 <= int(identity.split('-')[1]) <= 66}, expected)
        self.assertEqual(len(expected), 6)

    def test_preview_failure_has_exact_independent_required_obligations(self):
        expected = {
            'RBC-67-candidate-typed-start-failure-is-terminal-and-explicitly-retryable',
            'RBC-68-candidate-start-rejection-never-spins-or-retries-without-intent',
            'RBC-69-candidate-permission-edge-recovers-once-without-changing-hook',
            'RBC-70-session-typed-terminal-clears-frame-and-only-reattaches-rust-lease',
            'RBC-71-session-ipc-rejection-is-visible-without-client-start-or-stop',
            'RBC-72-candidate-transport-auto-retry-is-bounded-and-explicitly-reset',
            'RBC-73-candidate-target-change-and-cancel-isolate-late-start',
        }
        self.assertEqual(gate.PREVIEW_FAILURE_CHECKS, expected)
        self.assertEqual({identity for identity in gate.REQUIRED_CHECKS if 67 <= int(identity.split('-')[1]) <= 73}, expected)
        self.assertEqual(len(expected), 7)

    def test_preview_picker_has_exact_independent_required_obligations(self):
        expected = {
            'RBC-74-picker-requires-explicit-candidate-intent-and-shows-selected-frame',
            'RBC-75-picker-cancel-timeout-mismatch-busy-and-ipc-failure-stay-terminal',
            'RBC-76-picker-late-candidate-target-result-cannot-replace-next-preview',
            'RBC-77-session-picker-is-explicit-and-does-not-client-start-or-stop',
            'RBC-78-picker-late-session-result-only-detaches-old-session',
            'RBC-79-picker-candidate-cancel-releases-late-selected-lease',
            'RBC-80-real-tauri-picker-adapter-uses-only-explicit-narrow-select-ipc',
            'RBC-81-real-tauri-late-select-or-start-reply-cannot-replace-new-pull',
        }
        self.assertEqual(gate.PREVIEW_PICKER_CHECKS, expected)
        self.assertEqual({identity for identity in gate.REQUIRED_CHECKS if 74 <= int(identity.split('-')[1]) <= 81}, expected)
        self.assertEqual(len(expected), 8)

    def test_preview_subscriptions_have_exact_independent_required_obligations(self):
        expected = {
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
        }
        self.assertEqual(gate.PREVIEW_SUBSCRIPTION_CHECKS, expected)
        self.assertEqual({identity for identity in gate.REQUIRED_CHECKS if 82 <= int(identity.split('-')[1]) <= 97}, expected)
        self.assertEqual(len(expected), 16)

    def test_first_frame_resolution_has_exact_independent_obligations(self):
        expected = {
            'RBC-98-real-adapter-unresolved-target-first-frame-binds-once',
            'RBC-99-real-component-unresolved-candidate-and-session-decode-first-frame',
            'RBC-100-no-successful-frame-timeout-never-claims-an-expired-image',
        }
        self.assertEqual(gate.PREVIEW_FIRST_FRAME_CHECKS, expected)
        self.assertEqual({identity for identity in gate.REQUIRED_CHECKS if int(identity.split('-')[1]) >= 98}, expected)
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 97]
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_first_frame_case_requires_real_successful_nonempty_execution(self):
        invalid = ({'status': 'failed'}, {'status': 'not_run'}, {'status': 'skipped'},
                   {'skipped': True}, {'assertionCount': 0}, {'assertionCount': True},
                   {'assertionCount': -1}, {'id': 'RBC-101-unregistered-first-frame-case'})
        for identity in sorted(gate.PREVIEW_FIRST_FRAME_CHECKS):
            missing = copy.deepcopy(self.report)
            missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
            missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
            with self.subTest(identity=identity, changes='missing'):
                self.assertTrue(self.errors(missing))
            for changes in invalid:
                with self.subTest(identity=identity, changes=changes):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity).update(changes)
                    self.assertTrue(self.errors(report))

    def test_previous_eighty_one_checks_cannot_satisfy_preview_subscriptions(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 81]
        self.assertEqual(len(old['checks']), 81)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_preview_subscription_rejects_missing_failed_skipped_or_empty_execution(self):
        invalid = ({'status': 'failed'}, {'status': 'not_run'}, {'status': 'skipped'},
                   {'skipped': True}, {'skip': True}, {'todo': True}, {'cancelled': True},
                   {'assertionCount': 0}, {'assertionCount': -1}, {'assertionCount': True},
                   {'assertionCount': None}, {'assertionCount': '2'})
        for identity in sorted(gate.PREVIEW_SUBSCRIPTION_CHECKS):
            with self.subTest(identity=identity, changes='missing'):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))
            for changes in invalid:
                with self.subTest(identity=identity, changes=changes):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity).update(changes)
                    self.assertTrue(self.errors(report))

    def test_preview_subscription_duplicate_or_renamed_case_cannot_replace_an_obligation(self):
        for identity in sorted(gate.PREVIEW_SUBSCRIPTION_CHECKS):
            other = next(value for value in sorted(gate.PREVIEW_SUBSCRIPTION_CHECKS) if value != identity)
            for replacement in (other, 'RBC-98-unapproved-subscription-case'):
                with self.subTest(identity=identity, replacement=replacement):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity)['id'] = replacement
                    self.assertTrue(self.errors(report))

    def test_new_subscription_cases_cannot_displace_any_original_obligation(self):
        # B relocates candidate-owner assertions in the runner, not their IDs.
        # In particular RBC27/42/43/46 and all previous failures remain required.
        for identity in sorted(gate.REQUIRED_CHECKS - gate.PREVIEW_SUBSCRIPTION_CHECKS):
            with self.subTest(identity=identity):
                report = copy.deepcopy(self.report)
                next(row for row in report['checks'] if row['id'] == identity)['id'] = min(gate.PREVIEW_SUBSCRIPTION_CHECKS)
                self.assertTrue(self.errors(report))

    def test_previous_seventy_three_checks_cannot_satisfy_explicit_preview_picker(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 73]
        self.assertEqual(len(old['checks']), 73)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_preview_picker_obligation_rejects_missing_failed_or_empty_execution(self):
        invalid = ({'status': 'failed'}, {'status': 'skipped'}, {'skipped': True},
                   {'skip': True}, {'todo': True}, {'cancelled': True},
                   {'assertionCount': 0}, {'assertionCount': -1}, {'assertionCount': True})
        for identity in sorted(gate.PREVIEW_PICKER_CHECKS):
            with self.subTest(identity=identity):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))
            for changes in invalid:
                with self.subTest(identity=identity, changes=changes):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity).update(changes)
                    self.assertTrue(self.errors(report))

    def test_previous_sixty_six_checks_cannot_satisfy_preview_failures(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 66]
        self.assertEqual(len(old['checks']), 66)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_preview_failure_obligation_rejects_missing_failed_or_empty_execution(self):
        invalid = ({'status': 'failed'}, {'status': 'skipped'}, {'skipped': True},
                   {'skip': True}, {'todo': True}, {'cancelled': True},
                   {'assertionCount': 0}, {'assertionCount': -1}, {'assertionCount': True})
        for identity in sorted(gate.PREVIEW_FAILURE_CHECKS):
            with self.subTest(identity=identity):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))
            for changes in invalid:
                with self.subTest(identity=identity, changes=changes):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity).update(changes)
                    self.assertTrue(self.errors(report))

    def test_previous_sixty_checks_cannot_satisfy_wordcard_lifecycle(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 60]
        self.assertEqual(len(old['checks']), 60)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_wordcard_lifecycle_obligation_is_required(self):
        for identity in sorted(gate.WORDCARD_LIFECYCLE_CHECKS):
            with self.subTest(identity=identity):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))

    def test_each_wordcard_check_rejects_failure_skip_or_nonpositive_assertions(self):
        invalid = ({'status': 'failed'}, {'status': 'skipped'}, {'skipped': True},
                   {'skip': True}, {'todo': True}, {'cancelled': True},
                   {'assertionCount': 0}, {'assertionCount': -1}, {'assertionCount': True})
        for identity in sorted(gate.WORDCARD_LIFECYCLE_CHECKS):
            for changes in invalid:
                with self.subTest(identity=identity, changes=changes):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity).update(changes)
                    self.assertTrue(self.errors(report))

    def test_previous_fifty_checks_cannot_satisfy_rf07_history_recovery(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 50]
        self.assertEqual(len(old['checks']), 50)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_rf07_history_recovery_obligation_is_required(self):
        for identity in sorted(gate.HISTORY_RANGE_RECOVERY_CHECKS):
            with self.subTest(identity=identity):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))

    def test_each_rf07_check_rejects_failure_skip_or_nonpositive_assertions(self):
        invalid = ({'status': 'failed'}, {'status': 'skipped'}, {'skipped': True},
                   {'skip': True}, {'todo': True}, {'cancelled': True},
                   {'assertionCount': 0}, {'assertionCount': -1}, {'assertionCount': True})
        for identity in sorted(gate.HISTORY_RANGE_RECOVERY_CHECKS):
            for changes in invalid:
                with self.subTest(identity=identity, changes=changes):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity).update(changes)
                    self.assertTrue(self.errors(report))

    def test_rf07_duplicate_or_renamed_obligation_cannot_replace_missing_case(self):
        for identity in sorted(gate.HISTORY_RANGE_RECOVERY_CHECKS):
            for replacement in ('RBC-51-follow-expired-opens-one-new-range', 'RBC-61-unapproved-history-case'):
                if identity == replacement:
                    continue
                with self.subTest(identity=identity, replacement=replacement):
                    report = copy.deepcopy(self.report)
                    next(row for row in report['checks'] if row['id'] == identity)['id'] = replacement
                    self.assertTrue(self.errors(report))

    def test_previous_forty_seven_checks_do_not_satisfy_real_runtime_followup(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 47]
        self.assertEqual(len(old['checks']), 47)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_previous_twenty_eight_checks_do_not_satisfy_current_session_regressions(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 28]
        self.assertEqual(len(old['checks']), 28)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_previous_thirty_two_checks_do_not_satisfy_history_content_refresh(self):
        old = copy.deepcopy(self.report)
        old['checks'] = [row for row in old['checks'] if int(row['id'].split('-')[1]) <= 32]
        self.assertEqual(len(old['checks']), 32)
        old['assertionCount'] = sum(row['assertionCount'] for row in old['checks'])
        self.assertTrue(self.errors(old))

    def test_each_history_content_refresh_obligation_is_required(self):
        added = [row['id'] for row in self.report['checks'] if 33 <= int(row['id'].split('-')[1]) <= 38]
        self.assertEqual(len(added), 6)
        for identity in added:
            with self.subTest(identity=identity):
                missing = copy.deepcopy(self.report)
                missing['checks'] = [row for row in missing['checks'] if row['id'] != identity]
                missing['assertionCount'] = sum(row['assertionCount'] for row in missing['checks'])
                self.assertTrue(self.errors(missing))

    def test_valid_crc_and_inflate_cannot_stand_in_for_decoded_pixels(self):
        def chunk(kind, data):
            return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
        invalid = (b'\x89PNG\r\n\x1a\n'
                   + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
                   + chunk(b'IDAT', zlib.compress(b'not-pixel-scanlines'))
                   + chunk(b'IEND', b''))
        self.image.write_bytes(invalid)
        self.report['captures'][0]['sha256'] = hashlib.sha256(invalid).hexdigest()
        self.assertTrue(self.errors())

    def test_missing_or_extra_rbc_check_rejected(self):
        missing = copy.deepcopy(self.report)
        missing['checks'].pop()
        self.assertTrue(self.errors(missing))
        extra = copy.deepcopy(self.report)
        extra['checks'].append(dict(id='RBC-99-unauthorized', status='passed', assertionCount=2))
        self.assertTrue(self.errors(extra))

    def test_duplicate_replacing_required_rbc_rejected(self):
        self.report['checks'][-1] = copy.deepcopy(self.report['checks'][0])
        self.assertTrue(self.errors())

    def test_each_check_needs_actual_positive_assertion(self):
        for value in (0, -1, True, None, '2'):
            with self.subTest(value=value):
                self.report['checks'][0]['assertionCount'] = value
                self.assertTrue(self.errors())

    def test_total_assertions_cannot_be_missing_or_under_counted(self):
        for value in (0, 55, True, None):
            with self.subTest(value=value):
                self.report['assertionCount'] = value
                self.assertTrue(self.errors())

    def test_failed_skipped_and_cancelled_rows_rejected(self):
        for changes in ({'status': 'failed'}, {'status': 'skipped'}, {'skipped': True}, {'todo': True}, {'cancelled': 1}):
            report = copy.deepcopy(self.report)
            report['checks'][0].update(changes)
            self.assertTrue(self.errors(report))

    def test_old_nonce_or_incomplete_inventory_rejected(self):
        old = copy.deepcopy(self.report)
        old['nonce'] = 'previous-execution'
        self.assertTrue(self.errors(old))
        old = copy.deepcopy(self.report)
        old['sourceSha256'].pop(next(iter(old['sourceSha256'])))
        self.assertTrue(self.errors(old))

    def test_source_byte_drift_cannot_match_receipt(self):
        next(iter(self.root.glob('apps/desktop/src/*'))).write_text('// modified')
        self.assertNotEqual(gate.component_sources(self.root), self.sources)
        self.assertTrue(gate.validate_report(self.capture, self.report, self.nonce, gate.component_sources(self.root), 0, self.fixture_evidence))

    def test_hash_mismatch_rejected(self):
        self.report['captures'][0]['sha256'] = '0' * 64
        self.assertTrue(self.errors())

    def test_non_png_bytes_with_matching_hash_rejected(self):
        self.image.write_bytes(b'guard fixture is not a PNG')
        self.report['captures'][0]['sha256'] = hashlib.sha256(self.image.read_bytes()).hexdigest()
        self.assertTrue(self.errors())

    def test_png_corruption_and_trailing_bytes_rejected(self):
        for content in (png_bytes()[:-2], png_bytes() + b'extra', png_bytes().replace(b'IEND', b'FAIL')):
            self.image.write_bytes(content)
            self.report['captures'][0]['sha256'] = hashlib.sha256(content).hexdigest()
            self.assertTrue(self.errors())

    def test_unsafe_nested_absolute_and_symlink_capture_rejected(self):
        for name in ('../outside.png', '/outside.png', 'nested/test.png', r'..\outside.png'):
            report = copy.deepcopy(self.report)
            report['captures'][0]['file'] = name
            self.assertTrue(self.errors(report))
        outside = self.root / 'outside.png'
        outside.write_bytes(png_bytes())
        self.image.unlink()
        self.image.symlink_to(outside)
        self.assertTrue(self.errors())

    def test_no_capture_and_duplicate_capture_rejected(self):
        report = copy.deepcopy(self.report)
        report['captures'] = []
        self.assertTrue(self.errors(report))
        self.report['captures'].append(copy.deepcopy(self.report['captures'][0]))
        self.assertTrue(self.errors())

    def test_browser_receipt_cannot_claim_native_game_or_visual_acceptance(self):
        for key in ('nativeOrGameVerified', 'visualPixelAcceptance'):
            report = copy.deepcopy(self.report)
            report[key] = True
            self.assertTrue(self.errors(report))

    def test_nonzero_exit_and_producer_error_rejected(self):
        self.assertTrue(self.errors(code=1))
        self.report['errors'] = ['page crashed']
        self.assertTrue(self.errors())

    def test_fixed_command_fresh_nonce_paths_and_full_repository_drift(self):
        commands = []
        def controlled_subprocess(command, **kwargs):
            if command[:2] == ['node', '-e']:
                return real_subprocess_run(command, **kwargs)
            commands.append(command)
            if command == gate.FIXTURE_COMMAND:
                self.assertEqual(kwargs['timeout'], 600)
                guard_fixture_outputs(Path(kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_DIR']), kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_NONCE'], gate.fixture_sources(self.root), history_sources=gate.history_fixture_sources(self.root))
                return subprocess.CompletedProcess(command, 0, 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n', '')
            self.assertEqual(command[:2], ['node', 'apps/desktop/tests/visual/verify-reviewed-reading.mjs'])
            self.assertEqual(kwargs['timeout'], 600)
            out = Path(command[command.index('--output') + 1])
            self.assertFalse(out.exists())
            out.mkdir()
            (out / self.image.name).write_bytes(self.image.read_bytes())
            report = copy.deepcopy(self.report)
            report['nonce'] = command[command.index('--nonce') + 1]
            manifest = Path(command[command.index('--current-session-fixtures') + 1])
            report['currentSessionFixtures'] = gate.inspect_current_session_fixtures(manifest.parent, report['nonce'], gate.fixture_sources(self.root), expected_history_sources=gate.history_fixture_sources(self.root))
            (out / 'verification.json').write_text(json.dumps(report))
            # Drift outside desktop is also rejected by the full repository identity.
            (self.root / 'architecture.txt').write_text('changed during execution')
            return subprocess.CompletedProcess(command, 0, 'guard-only controlled child', '')
        with patch.object(gate.subprocess, 'run', side_effect=controlled_subprocess):
            result = gate.run_reading_components(self.root)
        self.assertEqual(result['status'], 'failed')
        self.assertTrue(any('Full repository' in error for error in result['errors']))
        self.assertNotEqual(result['source_tree_sha256_before'], result['source_tree_sha256_after'])
        self.assertFalse(result['product_packaged_native_game_tests_executed'])
        self.assertTrue((self.root / result['log']).is_file())
        self.assertEqual(len(commands), 2)

    def test_timeout_cannot_be_product_pass(self):
        with patch.object(gate.subprocess, 'run', side_effect=subprocess.TimeoutExpired(['node'], 600)):
            result = gate.run_reading_components(self.root)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['exit_code'], -1)
        self.assertFalse(result['product_component_tests_executed'])

    def test_accepted_controlled_child_uses_different_nonce_and_new_directory_each_time(self):
        # This only tests the wrapper contract; the mock is never a registered adapter.
        def controlled_subprocess(command, **kwargs):
            if command[:2] == ['node', '-e']:
                return real_subprocess_run(command, **kwargs)
            if command == gate.FIXTURE_COMMAND:
                guard_fixture_outputs(Path(kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_DIR']), kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_NONCE'], gate.fixture_sources(self.root), history_sources=gate.history_fixture_sources(self.root))
                return subprocess.CompletedProcess(command, 0, 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n', '')
            out = Path(command[command.index('--output') + 1])
            self.assertFalse(out.exists())
            out.mkdir()
            (out / self.image.name).write_bytes(self.image.read_bytes())
            report = copy.deepcopy(self.report)
            report['nonce'] = command[command.index('--nonce') + 1]
            manifest = Path(command[command.index('--current-session-fixtures') + 1])
            report['currentSessionFixtures'] = gate.inspect_current_session_fixtures(manifest.parent, report['nonce'], gate.fixture_sources(self.root), expected_history_sources=gate.history_fixture_sources(self.root))
            (out / 'verification.json').write_text(json.dumps(report))
            return subprocess.CompletedProcess(command, 0, 'guard-only controlled child', '')
        with patch.object(gate.subprocess, 'run', side_effect=controlled_subprocess):
            first = gate.run_reading_components(self.root)
            second = gate.run_reading_components(self.root)
        self.assertEqual(first['errors'], [])
        self.assertEqual(first['status'], 'passed')
        self.assertTrue(first['product_component_tests_executed'])
        self.assertFalse(first['product_packaged_native_game_tests_executed'])
        self.assertFalse(first['visualPixelAcceptance'])
        self.assertNotEqual(first['nonce'], second['nonce'])
        self.assertNotEqual(first['component_receipt'], second['component_receipt'])

    def rewrite_fixture_manifest(self, mutate):
        file = self.fixtures / 'manifest.json'
        value = json.loads(file.read_text())
        mutate(value)
        file.write_text(json.dumps(value))

    def inspect_fixtures(self):
        return gate.inspect_current_session_fixtures(self.fixtures, self.nonce, self.fixture_sources)

    def test_actual_fixture_manifest_and_all_four_payloads_are_required(self):
        manifest = self.fixtures / 'manifest.json'
        saved = manifest.read_bytes()
        manifest.unlink()
        with self.assertRaises(ValueError):
            self.inspect_fixtures()
        manifest.write_bytes(saved)
        (self.fixtures / (gate.CURRENT_SESSION_SCENARIOS[2] + '.json')).unlink()
        with self.assertRaises(ValueError):
            self.inspect_fixtures()

    def test_actual_fixture_nonce_query_and_producer_source_are_bound(self):
        original = (self.fixtures / 'manifest.json').read_bytes()
        for key, value in (('nonce', 'b' * 32), ('snapshotQuerySourceSha256', '0' * 64),
                           ('producerSourceSha256', '0' * 64), ('producer', 'fake'),
                           ('producerTarget', 'fake'), ('notLiveGameEvidence', False),
                           ('schemaVersion', True)):
            with self.subTest(key=key):
                (self.fixtures / 'manifest.json').write_bytes(original)
                self.rewrite_fixture_manifest(lambda manifest: manifest.update({key: value}))
                with self.assertRaises(ValueError):
                    self.inspect_fixtures()

    def test_actual_fixture_duplicate_missing_and_unknown_scenarios_rejected(self):
        original = (self.fixtures / 'manifest.json').read_bytes()
        mutations = [lambda m: m['entries'].pop(),
                     lambda m: m['entries'].__setitem__(3, copy.deepcopy(m['entries'][0])),
                     lambda m: m['entries'][0].update(scenario='05-fake'),
                     lambda m: m['entries'][0].pop('current')]
        for mutate in mutations:
            (self.fixtures / 'manifest.json').write_bytes(original)
            self.rewrite_fixture_manifest(mutate)
            with self.assertRaises(ValueError):
                self.inspect_fixtures()

    def test_actual_fixture_path_escape_absolute_nested_and_symlink_rejected(self):
        original = (self.fixtures / 'manifest.json').read_bytes()
        for path in ('../outside.json', '/outside.json', 'nested/fixture.json', r'..\outside.json'):
            (self.fixtures / 'manifest.json').write_bytes(original)
            self.rewrite_fixture_manifest(lambda m: m['entries'][0].update(path=path))
            with self.assertRaises(ValueError):
                self.inspect_fixtures()
        (self.fixtures / 'manifest.json').write_bytes(original)
        file = self.fixtures / (gate.CURRENT_SESSION_SCENARIOS[0] + '.json')
        outside = self.root / 'outside.json'
        outside.write_bytes(file.read_bytes())
        file.unlink()
        file.symlink_to(outside)
        with self.assertRaises(ValueError):
            self.inspect_fixtures()

    def test_actual_fixture_payload_hash_identity_and_current_relation_rejected(self):
        file = self.fixtures / (gate.CURRENT_SESSION_SCENARIOS[2] + '.json')
        original_payload = file.read_bytes()
        original_manifest = (self.fixtures / 'manifest.json').read_bytes()
        file.write_bytes(original_payload + b' ')
        with self.assertRaises(ValueError):
            self.inspect_fixtures()
        for key, value in (('sessionId', 'guard-old'), ('unitId', ''), ('textRevision', True)):
            file.write_bytes(original_payload)
            (self.fixtures / 'manifest.json').write_bytes(original_manifest)
            payload = json.loads(file.read_text())
            payload['current'][key] = value
            file.write_text(json.dumps(payload))
            self.rewrite_fixture_manifest(lambda m: m['entries'][2].update(sha256=gate.digest(file)))
            with self.assertRaises(ValueError):
                self.inspect_fixtures()

    def test_actual_fixture_not_live_evidence_and_fresh_output_are_enforced(self):
        self.assertNotIn('nativeOrGameVerified', self.fixture_evidence)
        self.assertNotIn('visualPixelAcceptance', self.fixture_evidence)
        (self.fixtures / 'unregistered.json').write_text('{}')
        with self.assertRaises(ValueError):
            self.inspect_fixtures()
        (self.fixtures / 'unregistered.json').unlink()
        future = max(file.stat().st_mtime_ns for file in self.fixtures.iterdir()) + 1
        with self.assertRaises(ValueError):
            gate.inspect_current_session_fixtures(self.fixtures, self.nonce, self.fixture_sources, future)

    def test_rust_actual_command_must_execute_one_unskipped_named_test(self):
        valid = 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n'
        gate.validate_fixture_command(valid, 0)
        for text, code in ((valid, 1), (valid.replace('1 passed', '0 passed'), 0),
                           (valid.replace('0 ignored', '1 ignored'), 0),
                           (valid.replace('... ok', '... ignored'), 0),
                           (valid.replace('exports_current_session_component_snapshots', 'other_test'), 0),
                           (valid + valid, 0)):
            with self.assertRaises(ValueError):
                gate.validate_fixture_command(text, code)

    def test_node_receipt_must_bind_actual_manifest_and_each_payload_hash(self):
        for key in ('fixtureManifestSha256', 'nonce', 'snapshotQuerySourceSha256', 'producerSourceSha256'):
            report = copy.deepcopy(self.report)
            report['currentSessionFixtures'][key] = 'different'
            self.assertTrue(self.errors(report))
        for change in ('missing', 'duplicate', 'hash', 'path'):
            report = copy.deepcopy(self.report)
            files = report['currentSessionFixtures']['files']
            if change == 'missing':
                files.pop()
            elif change == 'duplicate':
                files[-1] = files[0]
            else:
                files[0][change if change == 'path' else 'sha256'] = 'wrong'
            self.assertTrue(self.errors(report))
        report = copy.deepcopy(self.report)
        report.pop('currentSessionFixtures')
        self.assertTrue(self.errors(report))

    def test_node_receipt_extra_locator_fields_cannot_replace_actual_identity(self):
        report = copy.deepcopy(self.report)
        report['currentSessionFixtures']['manifestFile'] = '.build/guard-fixtures/manifest.json'
        for row in report['currentSessionFixtures']['files']:
            row['fileRootRelative'] = '.build/guard-fixtures/' + row['path']
        self.assertEqual(self.errors(report), [])
        report['currentSessionFixtures']['files'][0]['sha256'] = '0' * 64
        self.assertTrue(self.errors(report))

    def test_fixture_command_failure_prevents_node_execution(self):
        calls = []
        def failed(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 1, 'test result: FAILED', 'controlled guard failure')
        with patch.object(gate.subprocess, 'run', side_effect=failed):
            receipt = gate.run_reading_components(self.root)
        self.assertEqual(calls, [gate.FIXTURE_COMMAND])
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['actual_sqlite_runtime_fixture_test_executed'])
        self.assertFalse(receipt['product_component_tests_executed'])

    def test_node_cannot_modify_rust_fixture_bytes_during_consumption(self):
        calls = []
        def controlled(command, **kwargs):
            calls.append(command)
            if command == gate.FIXTURE_COMMAND:
                guard_fixture_outputs(Path(kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_DIR']), kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_NONCE'], gate.fixture_sources(self.root), history_sources=gate.history_fixture_sources(self.root))
                return subprocess.CompletedProcess(command, 0, 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n', '')
            manifest = Path(command[command.index('--current-session-fixtures') + 1])
            fixture = manifest.parent / (gate.CURRENT_SESSION_SCENARIOS[0] + '.json')
            fixture.write_bytes(fixture.read_bytes() + b' ')
            return subprocess.CompletedProcess(command, 0, 'guard-only tampering child', '')
        with patch.object(gate.subprocess, 'run', side_effect=controlled):
            receipt = gate.run_reading_components(self.root)
        self.assertEqual(len(calls), 2)
        self.assertEqual(receipt['status'], 'failed')
        self.assertTrue(any('fixture bytes' in error for error in receipt['errors']))
        self.assertFalse(receipt['product_component_tests_executed'])

    def test_rust_query_source_drift_prevents_node_execution(self):
        calls = []
        def controlled(command, **kwargs):
            calls.append(command)
            self.assertEqual(command, gate.FIXTURE_COMMAND)
            guard_fixture_outputs(Path(kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_DIR']), kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_NONCE'], gate.fixture_sources(self.root), history_sources=gate.history_fixture_sources(self.root))
            (self.root / gate.FIXTURE_SOURCE_PATHS['snapshotQuerySourceSha256']).write_text('// changed while Cargo ran')
            return subprocess.CompletedProcess(command, 0, 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n', '')
        with patch.object(gate.subprocess, 'run', side_effect=controlled):
            receipt = gate.run_reading_components(self.root)
        self.assertEqual(calls, [gate.FIXTURE_COMMAND])
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['product_component_tests_executed'])
        self.assertTrue(any('source changed' in error for error in receipt['errors']))

    def test_node_cannot_replace_fixture_directory_with_symlink(self):
        def controlled(command, **kwargs):
            if command == gate.FIXTURE_COMMAND:
                guard_fixture_outputs(Path(kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_DIR']), kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_NONCE'], gate.fixture_sources(self.root), history_sources=gate.history_fixture_sources(self.root))
                return subprocess.CompletedProcess(command, 0, 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n', '')
            fixtures = Path(command[command.index('--current-session-fixtures') + 1]).parent
            replacement = fixtures.with_name('moved-fixtures')
            fixtures.rename(replacement)
            fixtures.symlink_to(replacement, target_is_directory=True)
            return subprocess.CompletedProcess(command, 0, 'guard-only symlink replacement', '')
        with patch.object(gate.subprocess, 'run', side_effect=controlled):
            receipt = gate.run_reading_components(self.root)
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['product_component_tests_executed'])
        self.assertTrue(any('Symlink' in error for error in receipt['errors']))

    def test_malformed_collections_fail_closed(self):
        for key, value in (('checks', None), ('checks', [None]), ('captures', None), ('captures', [None]), ('schemaVersion', True)):
            report = copy.deepcopy(self.report)
            report[key] = value
            self.assertTrue(self.errors(report))

    def history_guard_case(self):
        """Metadata-only guard input; never substitute Runtime history DTOs."""
        index = getattr(self, '_history_guard_index', 0) + 1
        self._history_guard_index = index
        directory = self.root / '.build' / ('guard-history-' + str(index))
        sources = gate.history_fixture_sources(self.root)
        evidence = guard_fixture_outputs(directory, self.nonce, self.fixture_sources, history_sources=sources)
        return directory, sources, evidence

    def inspect_history_guard(self, directory, sources, not_before_ns=None):
        return gate.inspect_current_session_fixtures(directory, self.nonce, self.fixture_sources,
                                                     not_before_ns, expected_history_sources=sources)

    def rewrite_history_guard(self, directory, *, payload=None, raw=None):
        sidecar = directory / 'reading-status-history.json'
        if raw is None:
            raw = json.dumps(payload).encode()
        sidecar.write_bytes(raw)
        manifest = directory / 'manifest.json'
        value = json.loads(manifest.read_text())
        value['readingStatusHistory']['sha256'] = gate.digest(sidecar)
        manifest.write_text(json.dumps(value))

    def history_report_errors(self, evidence, descriptor):
        report = copy.deepcopy(self.report)
        report['currentSessionFixtures'] = copy.deepcopy(evidence)
        report['currentSessionFixtures']['readingStatusHistory'] = descriptor
        return gate.validate_report(self.capture, report, self.nonce, self.sources, 0, evidence)

    def test_history_sources_are_exact_independent_eight_current_files(self):
        self.assertEqual(gate.READING_HISTORY_SOURCE_PATHS, HISTORY_SOURCE_PATHS)
        expected = {path: hashlib.sha256((self.root / path).read_bytes()).hexdigest()
                    for path in sorted(HISTORY_SOURCE_PATHS)}
        self.assertEqual(gate.history_fixture_sources(self.root), expected)
        path = self.root / 'crates/app-core/src/domain/translation_status.rs'
        original = path.read_bytes()
        path.unlink()
        with self.assertRaises((ValueError, OSError)):
            gate.history_fixture_sources(self.root)
        outside = self.root / 'untrusted-history-source.rs'
        outside.write_bytes(original)
        path.symlink_to(outside)
        with self.assertRaises(ValueError):
            gate.history_fixture_sources(self.root)

    def test_history_sidecar_valid_metadata_keeps_original_four_scenarios(self):
        directory, sources, evidence = self.history_guard_case()
        self.assertEqual(evidence['readingStatusHistory'], {
            'path': 'reading-status-history.json',
            'sha256': gate.digest(directory / 'reading-status-history.json'),
            'sourceSha256': sources,
        })
        self.assertEqual(len(evidence['files']), 4)
        self.assertEqual({row['scenario'] for row in evidence['files']}, set(gate.CURRENT_SESSION_SCENARIOS))
        self.assertEqual(self.history_report_errors(evidence, copy.deepcopy(evidence['readingStatusHistory'])), [])
        payload = json.loads((directory / 'reading-status-history.json').read_text())
        payload['guardOnlyOpaqueBody'] = {'notAProductDTO': True}
        self.rewrite_history_guard(directory, payload=payload)
        self.assertEqual(self.inspect_history_guard(directory, sources)['readingStatusHistory']['sourceSha256'], sources,
                         'The adapter checks identity without reproducing translation/history business rules')

    def test_history_strict_call_requires_sidecar_and_legacy_helper_cannot_accept_extension(self):
        self.assertEqual(self.inspect_fixtures(), self.fixture_evidence)
        with self.assertRaises(ValueError):
            gate.inspect_current_session_fixtures(self.fixtures, self.nonce, self.fixture_sources,
                                                 expected_history_sources=gate.history_fixture_sources(self.root))
        directory, sources, _ = self.history_guard_case()
        with self.assertRaises(ValueError):
            gate.inspect_current_session_fixtures(directory, self.nonce, self.fixture_sources)
        (directory / 'reading-status-history.json').unlink()
        with self.assertRaises(ValueError):
            self.inspect_history_guard(directory, sources)

    def test_history_manifest_declaration_is_exact_shape(self):
        directory, sources, _ = self.history_guard_case()
        file = directory / 'manifest.json'
        original = json.loads(file.read_text())
        values = [None, True, 'reading-status-history.json', [], {},
                  {'path': 'reading-status-history.json'}, {'sha256': 'a' * 64},
                  {**original['readingStatusHistory'], 'unknown': True},
                  {**original['readingStatusHistory'], 'sha256': True},
                  {**original['readingStatusHistory'], 'sha256': 'not-a-hash'}]
        for value in values:
            with self.subTest(value=value):
                manifest = copy.deepcopy(original)
                manifest['readingStatusHistory'] = value
                file.write_text(json.dumps(manifest))
                with self.assertRaises(ValueError):
                    self.inspect_history_guard(directory, sources)

    def test_history_manifest_path_is_only_exact_local_basename(self):
        directory, sources, _ = self.history_guard_case()
        file = directory / 'manifest.json'
        original = json.loads(file.read_text())
        for path in ('../reading-status-history.json', '/reading-status-history.json',
                     'nested/reading-status-history.json', r'..\reading-status-history.json',
                     'another.json', 'reading-status-history.json/', '', None, True):
            with self.subTest(path=path):
                manifest = copy.deepcopy(original)
                manifest['readingStatusHistory']['path'] = path
                file.write_text(json.dumps(manifest))
                with self.assertRaises(ValueError):
                    self.inspect_history_guard(directory, sources)

    def test_history_sidecar_symlink_empty_oversize_and_stale_rejected(self):
        directory, sources, _ = self.history_guard_case()
        sidecar = directory / 'reading-status-history.json'
        original = sidecar.read_bytes()
        outside = self.root / 'guard-external-history.json'
        outside.write_bytes(original)
        sidecar.unlink()
        sidecar.symlink_to(outside)
        with self.assertRaises(ValueError):
            self.inspect_history_guard(directory, sources)
        sidecar.unlink()
        for raw in (b'', b' ' * (8 * 1024 * 1024 + 1)):
            self.rewrite_history_guard(directory, raw=raw)
            with self.assertRaises(ValueError):
                self.inspect_history_guard(directory, sources)
        self.rewrite_history_guard(directory, raw=original)
        threshold = min(file.stat().st_mtime_ns for file in directory.iterdir() if file != sidecar)
        os.utime(sidecar, ns=(1, 1))
        with self.assertRaises(ValueError):
            self.inspect_history_guard(directory, sources, threshold)

    def test_history_directory_rejects_extra_file_and_directory(self):
        for is_directory in (False, True):
            directory, sources, _ = self.history_guard_case()
            extra = directory / 'unregistered-output'
            if is_directory:
                extra.mkdir()
            else:
                extra.write_text('{}')
            with self.assertRaises(ValueError):
                self.inspect_history_guard(directory, sources)

    def test_history_payload_hash_and_all_metadata_are_bound(self):
        directory, sources, _ = self.history_guard_case()
        sidecar = directory / 'reading-status-history.json'
        original = json.loads(sidecar.read_text())
        sidecar.write_bytes(sidecar.read_bytes() + b' ')
        with self.assertRaises(ValueError):
            self.inspect_history_guard(directory, sources)
        mutations = [('schemaVersion', True), ('schemaVersion', 2), ('schemaVersion', '1'),
                     ('kind', 'different_kind'), ('nonce', 'b' * 32), ('nonce', None),
                     ('sourceSha256', None), ('sourceSha256', []), ('sourceSha256', {})]
        for key, value in mutations:
            with self.subTest(key=key, value=value):
                payload = copy.deepcopy(original)
                payload[key] = value
                self.rewrite_history_guard(directory, payload=payload)
                with self.assertRaises(ValueError):
                    self.inspect_history_guard(directory, sources)
        for key in ('schemaVersion', 'kind', 'nonce', 'sourceSha256'):
            payload = copy.deepcopy(original)
            payload.pop(key)
            self.rewrite_history_guard(directory, payload=payload)
            with self.assertRaises(ValueError):
                self.inspect_history_guard(directory, sources)

    def test_history_duplicate_raw_keys_including_nested_source_map_rejected(self):
        directory, sources, _ = self.history_guard_case()
        payload = json.loads((directory / 'reading-status-history.json').read_text())
        raw = json.dumps(payload)
        first_source = sorted(sources)[0]
        source_pair = json.dumps(first_source) + ': ' + json.dumps(sources[first_source])
        for duplicated in (raw.replace('"schemaVersion": 1', '"schemaVersion": 1, "schemaVersion": 1', 1),
                           raw.replace(source_pair, source_pair + ', ' + source_pair, 1)):
            self.assertNotEqual(duplicated, raw)
            self.rewrite_history_guard(directory, raw=duplicated.encode())
            with self.assertRaises(ValueError):
                self.inspect_history_guard(directory, sources)

    def test_history_exact_eight_source_keys_hashes_and_original_two_cross_binding(self):
        directory, sources, _ = self.history_guard_case()
        original = json.loads((directory / 'reading-status-history.json').read_text())
        variants = []
        missing = dict(sources); missing.pop(sorted(sources)[0]); variants.append(missing)
        variants.append({**sources, 'crates/app-core/src/unapproved.rs': 'a' * 64})
        for value in ('0' * 64, True, 'not-a-hash'):
            variants.append({**sources, sorted(sources)[0]: value})
        for wrong in variants:
            payload = {**original, 'sourceSha256': wrong}
            self.rewrite_history_guard(directory, payload=payload)
            with self.assertRaises(ValueError):
                self.inspect_history_guard(directory, sources)
            self.rewrite_history_guard(directory, payload=original)
            with self.assertRaises(ValueError):
                self.inspect_history_guard(directory, wrong)
        source_path = gate.FIXTURE_SOURCE_PATHS['snapshotQuerySourceSha256']
        conflicting = {**sources, source_path: '0' * 64}
        self.rewrite_history_guard(directory, payload={**original, 'sourceSha256': conflicting})
        with self.assertRaises(ValueError):
            self.inspect_history_guard(directory, conflicting)

    def test_history_receipt_requires_path_hash_sources_and_rejects_unknown_identity(self):
        _, _, evidence = self.history_guard_case()
        original = evidence['readingStatusHistory']
        variants = [None, [], {}, {**original, 'path': 'wrong.json'},
                    {**original, 'sha256': '0' * 64}, {**original, 'sourceSha256': None},
                    {**original, 'sourceSha256': {}}, {**original, 'unknownIdentity': 'not-approved'}]
        for key in ('path', 'sha256', 'sourceSha256'):
            value = copy.deepcopy(original); value.pop(key); variants.append(value)
        first_source = sorted(original['sourceSha256'])[0]
        for sources in ({**original['sourceSha256'], first_source: '0' * 64},
                        {**original['sourceSha256'], 'unapproved.rs': 'a' * 64}):
            variants.append({**original, 'sourceSha256': sources})
        missing_source = dict(original['sourceSha256']); missing_source.pop(first_source)
        variants.append({**original, 'sourceSha256': missing_source})
        for descriptor in variants:
            with self.subTest(descriptor=descriptor):
                self.assertTrue(self.history_report_errors(evidence, descriptor))
        report = copy.deepcopy(self.report)
        report['currentSessionFixtures'] = copy.deepcopy(evidence)
        report['currentSessionFixtures'].pop('readingStatusHistory')
        self.assertTrue(gate.validate_report(self.capture, report, self.nonce, self.sources, 0, evidence))
        report = copy.deepcopy(self.report)
        report['currentSessionFixtures']['readingStatusHistory'] = copy.deepcopy(original)
        self.assertTrue(self.errors(report), 'Legacy expected evidence cannot accept an unbound extra sidecar')

    def test_history_receipt_locator_is_optional_and_cannot_replace_identity(self):
        _, _, evidence = self.history_guard_case()
        original = evidence['readingStatusHistory']
        located = {**original, 'fileRootRelative': '.build/guard-fixtures/reading-status-history.json'}
        self.assertEqual(self.history_report_errors(evidence, located), [])
        self.assertEqual(self.history_report_errors(evidence, copy.deepcopy(original)), [])
        for key in ('path', 'sha256', 'sourceSha256'):
            descriptor = copy.deepcopy(located); descriptor.pop(key)
            self.assertTrue(self.history_report_errors(evidence, descriptor))
        for locator in (None, True, [], {}):
            self.assertTrue(self.history_report_errors(evidence, {**original, 'fileRootRelative': locator}))

    def run_history_guard_child(self, *, mutation=None, omit_sidecar=False, during_fixture=False):
        """A mocked command rejects bad evidence; never run as a product adapter."""
        calls = []
        def controlled(command, **kwargs):
            if command[:2] == ['node', '-e']:
                return real_subprocess_run(command, **kwargs)
            calls.append(command)
            if command == gate.FIXTURE_COMMAND:
                directory = Path(kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_DIR'])
                guard_fixture_outputs(directory, kwargs['env']['LUNA_CURRENT_SESSION_FIXTURE_NONCE'],
                                      gate.fixture_sources(self.root),
                                      history_sources=None if omit_sidecar else gate.history_fixture_sources(self.root))
                if mutation and during_fixture:
                    mutation(directory)
                return subprocess.CompletedProcess(command, 0, 'test exports_current_session_component_snapshots ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out;\n', '')
            out = Path(command[command.index('--output') + 1])
            out.mkdir()
            (out / self.image.name).write_bytes(self.image.read_bytes())
            report = copy.deepcopy(self.report)
            report['nonce'] = command[command.index('--nonce') + 1]
            directory = Path(command[command.index('--current-session-fixtures') + 1]).parent
            report['currentSessionFixtures'] = gate.inspect_current_session_fixtures(
                directory, report['nonce'], gate.fixture_sources(self.root),
                expected_history_sources=gate.history_fixture_sources(self.root))
            if mutation and not during_fixture:
                mutation(directory)
            (out / 'verification.json').write_text(json.dumps(report))
            return subprocess.CompletedProcess(command, 0, 'guard-only metadata child', '')
        # Hold the broad source digest fixed only in these guard negatives, so
        # source/sidecar reinspection must independently catch the mutation.
        with patch.object(gate, 'tree_identity', return_value='guard-fixed-source-tree'), \
                patch.object(gate.subprocess, 'run', side_effect=controlled):
            result = gate.run_reading_components(self.root)
        return calls, result

    def test_history_real_run_cannot_use_legacy_four_only_fixture(self):
        calls, receipt = self.run_history_guard_child(omit_sidecar=True)
        self.assertEqual(calls, [gate.FIXTURE_COMMAND])
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['product_component_tests_executed'])

    def test_history_node_sidecar_byte_and_resigned_manifest_changes_rejected(self):
        for resign in (False, True):
            def mutate(directory):
                sidecar = directory / 'reading-status-history.json'
                if resign:
                    payload = json.loads(sidecar.read_text())
                    payload['guardOpaqueBodyChanged'] = True
                    self.rewrite_history_guard(directory, payload=payload)
                else:
                    sidecar.write_bytes(sidecar.read_bytes() + b' ')
            calls, receipt = self.run_history_guard_child(mutation=mutate)
            self.assertEqual(len(calls), 2)
            self.assertEqual(receipt['status'], 'failed')
            self.assertTrue(any('inspection' in error or 'fixture bytes' in error for error in receipt['errors']))
            self.assertFalse(receipt['product_component_tests_executed'])

    def test_history_all_eight_source_files_rechecked_after_node(self):
        extra_source = self.root / 'crates/app-core/src/domain/translation_status.rs'
        calls, receipt = self.run_history_guard_child(mutation=lambda _: extra_source.write_text('// changed while Node ran'))
        self.assertEqual(len(calls), 2)
        self.assertEqual(receipt['status'], 'failed')
        self.assertTrue(any('source changed' in error for error in receipt['errors']))
        self.assertFalse(receipt['product_component_tests_executed'])

    def test_history_all_eight_source_files_rechecked_before_node(self):
        extra_source = self.root / 'crates/app-core/src/domain/translation_status.rs'
        calls, receipt = self.run_history_guard_child(mutation=lambda _: extra_source.write_text('// changed while Cargo ran'), during_fixture=True)
        self.assertEqual(calls, [gate.FIXTURE_COMMAND])
        self.assertEqual(receipt['status'], 'failed')
        self.assertTrue(any('source changed' in error for error in receipt['errors']))
        self.assertFalse(receipt['product_component_tests_executed'])

    def test_history_node_extra_output_and_sidecar_symlink_rejected(self):
        for symlink in (False, True):
            def mutate(directory):
                if symlink:
                    sidecar = directory / 'reading-status-history.json'
                    outside = self.root / 'guard-node-sidecar.json'
                    outside.write_bytes(sidecar.read_bytes()); sidecar.unlink(); sidecar.symlink_to(outside)
                else:
                    (directory / 'unregistered-output').mkdir()
            calls, receipt = self.run_history_guard_child(mutation=mutate)
            self.assertEqual(len(calls), 2)
            self.assertEqual(receipt['status'], 'failed')
            self.assertFalse(receipt['product_component_tests_executed'])


if __name__ == '__main__':
    unittest.main()
