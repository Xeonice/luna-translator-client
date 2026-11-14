"""Negative checks for finite visual evidence; guard success is not UI approval."""
import copy
import hashlib
from pathlib import Path
import tempfile
import unittest

from preview_visual_evidence import CASES, DETAILS, VARIANTS, artifact, validate_policy


class PreviewVisualEvidenceTests(unittest.TestCase):
    def receipt(self):
        return {'schemaVersion': 1, 'kind': 'current-preview-visuals/v1',
                'executionNonce': 'a' * 32, 'referenceApproval': 'pending_independent_review',
                'productAcceptance': 'blocked', 'packagedNativeVerified': False,
                'allStateAdapterRegistered': False, 'skippedAssertions': 0,
                'startedAt': '2026-11-04T00:00:00Z', 'finishedAt': '2026-11-04T00:00:01Z',
                'checks': [{'id': x, 'status': 'passed', 'assertionCount': 1} for x in CASES],
                'assertionCount': len(CASES), 'comparisons': [{'id': x} for x in VARIANTS],
                'detailComparisons': [{'id': x} for x in DETAILS], 'sourceDrift': [],
                'pageErrors': [], 'requestViolations': [], 'cspViolations': [],
                'behaviorStatus': 'passed', 'pixelStatus': 'passed', 'status': 'passed'}

    def validate(self, r):
        return validate_policy(r, 'a' * 32, 1793750399, 1793750402)

    def test_valid_finite_policy_is_not_native_approval(self):
        self.assertEqual(self.validate(self.receipt()), [])

    def test_failed_pixels_and_blocked_reference_cannot_be_pass(self):
        for value in ('failed', 'blocked_missing_source_state'):
            with self.subTest(value=value):
                r = self.receipt(); r['pixelStatus'] = value
                errors = self.validate(r)
                self.assertTrue(any('strict' in e for e in errors))
                self.assertTrue(any('hides' in e for e in errors))

    def test_missing_reordered_and_duplicate_cases_rejected(self):
        for key in ('checks', 'comparisons', 'detailComparisons'):
            for mode in ('missing', 'reordered', 'duplicate'):
                with self.subTest(key=key, mode=mode):
                    r = self.receipt(); rows = r[key]
                    r[key] = rows[:-1] if mode == 'missing' else list(reversed(rows)) if mode == 'reordered' else rows + [copy.deepcopy(rows[-1])]
                    self.assertTrue(self.validate(r))

    def test_capability_and_reference_promotion_rejected(self):
        for key, value in (('packagedNativeVerified', True), ('allStateAdapterRegistered', True),
                           ('productAcceptance', 'passed'), ('referenceApproval', 'approved_by_user'),
                           ('packagedNativeVerified', 0), ('schemaVersion', True)):
            with self.subTest(key=key, value=value):
                r = self.receipt(); r[key] = value; self.assertTrue(self.validate(r))

    def test_stale_execution_rejected(self):
        r = self.receipt(); r['executionNonce'] = 'b' * 32; self.assertTrue(self.validate(r))
        r = self.receipt(); r['finishedAt'] = '2026-11-05T00:00:00Z'; self.assertTrue(self.validate(r))

    def test_actual_assertions_and_runtime_errors_required(self):
        for key in ('sourceDrift', 'pageErrors', 'requestViolations', 'cspViolations'):
            r = self.receipt(); r[key] = ['unexpected']; self.assertTrue(self.validate(r))
        for value in (0, -1, True):
            r = self.receipt(); r['checks'][0]['assertionCount'] = value; self.assertTrue(self.validate(r))

    def test_artifact_tamper_and_path_escape_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp); p = out / 'frame.png'; p.write_bytes(b'original')
            descriptor = {'path': 'frame.png', 'bytes': 8, 'sha256': hashlib.sha256(b'original').hexdigest()}
            self.assertEqual(artifact(out, descriptor), p.resolve())
            p.write_bytes(b'tampered')
            with self.assertRaises(ValueError): artifact(out, descriptor)
            descriptor['path'] = '../outside'
            with self.assertRaises(ValueError): artifact(out, descriptor)


if __name__ == '__main__': unittest.main()
