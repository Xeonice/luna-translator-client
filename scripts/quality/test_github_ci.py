"""Test pinned transport boundaries with real archives and file effects."""
import copy
import hashlib
import json
from pathlib import Path
import re
import stat
import struct
import subprocess
import tempfile
import textwrap
import unittest
from unittest.mock import patch
import zipfile

import github_ci as ci


class PinnedTransportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='luna-ci-transport-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'source'
        self.source.mkdir()

    def zipped(self, members):
        target = self.root / 'input.zip'
        with zipfile.ZipFile(target, 'w') as archive:
            for member, value in members:
                archive.writestr(member, value)
        return target

    def test_real_archive_round_trip_and_exact_manifest(self):
        (self.source / 'data').write_bytes(b'original bytes')
        bundle = self.root / 'bundle.zip'
        lock = ci.archive(self.source, ['data'], bundle)
        out = self.root / 'restored'
        out.mkdir()
        self.assertEqual(ci.restore(out, bundle, expected=lock['files']), ['data'])
        self.assertEqual((out / 'data').read_bytes(), b'original bytes')

    def test_rejected_inventory_and_hash_leave_no_partial_source(self):
        bundle = self.zipped([('one', b'first'), ('two', b'last')])
        expected = {key: {'sha256': hashlib.sha256(value).hexdigest(), 'size': len(value)}
                    for key, value in [('one', b'first'), ('two', b'last')]}
        expected['two']['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            ci.restore(self.source, bundle, expected=expected)
        self.assertFalse((self.source / 'one').exists())
        with self.assertRaisesRegex(ValueError, 'inventory'):
            ci.restore(self.source, bundle, expected={'one': expected['one']})

    def test_traversal_and_symlink_are_rejected_before_write(self):
        for member in ['../escape', '/absolute', 'folder\\escape']:
            with self.subTest(member=member):
                bundle = self.zipped([(member, b'unsafe')])
                with self.assertRaises(ValueError):
                    ci.restore(self.source, bundle)
        info = zipfile.ZipInfo('link')
        info.external_attr = (stat.S_IFLNK | 0o777) << 16
        bundle = self.zipped([(info, b'../secret')])
        with self.assertRaises(ValueError):
            ci.restore(self.source, bundle)
        self.assertEqual(list(self.source.iterdir()), [])

    def test_duplicate_members_and_wrong_evidence_prefix_rejected(self):
        with self.assertWarns(UserWarning):
            bundle = self.zipped([('duplicate', b'1'), ('duplicate', b'2')])
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            ci.restore(self.source, bundle)
        bundle = self.zipped([('resources/unrelated', b'1')])
        with self.assertRaisesRegex(ValueError, 'unsafe member'):
            ci.restore(self.source, bundle, prefix=ci.EVIDENCE_PREFIX)

    def test_alias_member_names_cannot_overwrite_one_authority_file(self):
        for alias in ['folder/./review.json', 'folder//review.json', 'folder/Review.json']:
            bundle = self.zipped([('folder/review.json', b'approved'), (alias, b'replacement')])
            with self.subTest(alias=alias), self.assertRaises(ValueError):
                ci.restore(self.source, bundle)
        self.assertEqual(list(self.source.iterdir()), [])

    def test_evidence_transport_is_bounded_before_decompression(self):
        bundle = self.zipped([('item', b'1')])
        raw = bytearray(bundle.read_bytes())
        central = raw.index(b'PK\x01\x02')
        struct.pack_into('<I', raw, central + 24, 65 * 1024 * 1024)
        bundle.write_bytes(raw)
        with self.assertRaisesRegex(ValueError, 'bounded transport size'):
            ci.restore(self.source, bundle)

    def test_download_requires_fixed_repository_https_and_hash(self):
        for url in ['http://github.com/Xeonice/luna-translator-client/releases/download/x/a.zip',
                    'https://example.com/a.zip', 'https://github.com/other/repo/releases/download/x/a.zip']:
            with self.subTest(url=url), self.assertRaises(ValueError):
                ci.download(url, 'a' * 64, self.root / 'download')
        with self.assertRaises(ValueError):
            ci.download('https://github.com/Xeonice/luna-translator-client/releases/download/x/a.zip',
                        '', self.root / 'download')

    def test_resource_lock_cannot_change_the_product_manifest(self):
        directory = self.source / 'resources/language'
        directory.mkdir(parents=True)
        entries = []
        expected = {}
        for p in ci.LARGE_RESOURCES:
            f = self.source / p
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_bytes(b'known resource')
            row = {'sha256': ci.sha(f), 'size': f.stat().st_size}
            expected[p] = row
            entries.append({'path': p.removeprefix('resources/'), **row})
        (directory / 'manifest.json').write_text(json.dumps({'files': entries}))
        self.assertFalse(ci.restore_resources(self.source, {'files': expected})['restored'])
        altered = copy.deepcopy(expected)
        altered[ci.LARGE_RESOURCES[0]]['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'existing product language manifest'):
            ci.restore_resources(self.source, {'files': altered})

    def test_cold_compiler_staging_contains_real_verified_bytes(self):
        directory = self.source / 'resources/language'
        directory.mkdir(parents=True)
        resource = directory / 'dictionary-data'
        resource.write_bytes(b'actual pinned source')
        manifest = {'files': [{'path': 'language/dictionary-data', 'size': resource.stat().st_size,
                               'sha256': ci.sha(resource)}]}
        (directory / 'manifest.json').write_text(json.dumps(manifest))
        resource.write_bytes(b'changed source')
        with self.assertRaisesRegex(ValueError, 'differs from product manifest'):
            ci.stage_compiler_resources(self.source)
        self.assertFalse((self.source / 'apps/desktop/src-tauri/resources').exists())
        resource.write_bytes(b'actual pinned source')
        result = ci.stage_compiler_resources(self.source)
        self.assertFalse(result['application_runtime_qualified'])
        staged = self.source / 'apps/desktop/src-tauri/resources/language/dictionary-data'
        self.assertEqual(staged.read_bytes(), resource.read_bytes())


class TrustedWorkflowTests(unittest.TestCase):
    def test_report_never_borrows_another_change_guard_run(self):
        from render_gate_report import render
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            old = root / '.build/quality/effective-gates-implementation/effectiveness/verification.json'
            old.parent.mkdir(parents=True)
            old.write_text(json.dumps({'counts': {'previous-change-only': 32}, 'scenarios': []}))
            report = render(root, {'change_id': 'new-change', 'gates': [], 'status': 'passed'})
            self.assertNotIn('previous-change-only', report.read_text())

    def run_script(self, index, *, pr=True, admission='success', engineering='success',
                   mode='normal', review_head=None, api_head=None, permission='admin', negative=False):
        workflow = (ci.ROOT / '.github/workflows/engineering-gates.yml').read_text()
        blocks = re.findall(r'^          script: \|\n((?:^            .*\n|^\n)+)', workflow, re.M)
        self.assertEqual(len(blocks), 2)
        head, base = 'a' * 40, 'b' * 40
        context = {'repo': {'owner': 'Xeonice', 'repo': 'luna-translator-client'},
                   'actor': 'reviewer', 'serverUrl': 'https://github.com', 'runId': '1',
                   'payload': {'inputs': {'head': head, 'base': base, 'negative_probe': str(negative).lower()}}}
        if pr:
            context['payload']['pull_request'] = {'number': 1, 'head': {'sha': head, 'repo': {'full_name': 'Xeonice/luna-translator-client'}}, 'base': {'sha': base}}
        payload = {'script': textwrap.dedent(blocks[index]), 'context': context, 'permission': permission,
                   'api_head': api_head or head, 'env': {'REVIEW_HEAD': head if review_head is None else review_head,
                   'REVIEW_HASH': 'c' * 64, 'REVIEW_URL': 'https://github.com/Xeonice/luna-translator-client/releases/download/x/review.zip',
                   'ADMISSION_RESULT': admission, 'ENGINEERING_RESULT': engineering,
                   'ADMITTED_HEAD': head, 'ADMISSION_MODE': mode}}
        code = r'''
import fs from 'node:fs';
const p=JSON.parse(fs.readFileSync(0,'utf8')); Object.assign(process.env,p.env);
const calls=[],outputs={};const core={setOutput:(k,v)=>outputs[k]=v,info:()=>{}};
const github={rest:{repos:{getCommit:async()=>({}),getCollaboratorPermissionLevel:async()=>({data:{permission:p.permission}})},checks:{create:async v=>{calls.push(v);return {data:{id:1,app:{id:15368}}};}},pulls:{get:async()=>({data:{head:{sha:p.api_head}}})}}};
const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor;
try {await new AsyncFunction('github','core','context',p.script)(github,core,p.context);process.stdout.write(JSON.stringify({passed:true,calls,outputs}));}
catch(e){process.stdout.write(JSON.stringify({passed:false,error:e.message,calls,outputs}));}
'''
        actual = subprocess.run(['node', '--input-type=module', '-e', code], input=json.dumps(payload),
                                capture_output=True, text=True, timeout=20, check=True)
        return json.loads(actual.stdout)

    def test_real_admission_script_rejects_unapproved_head_and_nonadmin_probe(self):
        self.assertTrue(self.run_script(0)['passed'])
        self.assertFalse(self.run_script(0, review_head='d' * 40)['passed'])
        self.assertFalse(self.run_script(0, pr=True, negative=True, review_head='d' * 40)['passed'])
        self.assertFalse(self.run_script(0, pr=False, negative=True, permission='write')['passed'])
        self.assertEqual(self.run_script(0, pr=False, negative=True)['outputs']['mode'], 'negative_probe')

    def test_real_final_script_cannot_turn_skip_failure_or_stale_head_green(self):
        self.assertEqual(self.run_script(1)['calls'][0]['conclusion'], 'success')
        for state in ['failure', 'skipped', 'cancelled', '', 'timed_out']:
            with self.subTest(state=state):
                self.assertEqual(self.run_script(1, engineering=state)['calls'][0]['conclusion'], 'failure')
                self.assertEqual(self.run_script(1, admission=state)['calls'][0]['conclusion'], 'failure')
        self.assertEqual(self.run_script(1, api_head='f' * 40)['calls'][0]['conclusion'], 'failure')
        self.assertEqual(self.run_script(1, mode='negative_probe')['calls'][0]['conclusion'], 'failure')
        self.assertEqual(self.run_script(1, pr=False, negative=True)['calls'], [])

    def test_write_authority_is_separate_from_candidate_execution(self):
        workflow = (ci.ROOT / '.github/workflows/engineering-gates.yml').read_text()
        admission, rest = workflow.split('\n  engineering:', 1)
        engineering, publisher = rest.split('\n  final-status:', 1)
        self.assertNotIn('actions/checkout', admission)
        self.assertNotIn('actions/checkout', publisher)
        self.assertNotIn('checks: write', engineering)
        self.assertIn('persist-credentials: false', engineering)
        self.assertIn('ref: ${{ needs.admission.outputs.head }}', engineering)
        self.assertIn('if: always()', publisher)
        self.assertIn('pull_request_target:', workflow)


if __name__ == '__main__':
    unittest.main()
