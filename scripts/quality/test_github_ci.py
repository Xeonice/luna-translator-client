"""Test pinned transport boundaries with real archives and file effects."""
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import struct
import subprocess
import sys
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


class ProductionOutputEnvironmentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='luna-ci-output-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'source.txt').write_text('stable controlled command input')
        context = self.root / '.codex/gate-context.json'; context.parent.mkdir()
        context.write_text(json.dumps({'change_id': 'controlled-output'}))
        baseline = self.root / '.build/source.json'; baseline.parent.mkdir()
        baseline.write_text(json.dumps({'files': {'source.txt': ci.sha(self.root / 'source.txt')}}))
        metadata = self.root / 'openspec/changes/controlled-output/verification.json'
        metadata.parent.mkdir(parents=True)
        metadata.write_text(json.dumps({'allowed_paths': ['scripts/quality/**'],
                                       'source_snapshot': '.build/source.json',
                                       'effective_gate_external_evidence': '.build/quality/external.json'}))
        lock = self.root / '.github/ci-evidence.lock.json'; lock.parent.mkdir()
        lock.write_text(json.dumps({'proof_path': '.build/quality/controlled-hook-proof.json'}))

    def child_environment(self, *, fail=False):
        # An actual child process observes inherited variables. This substitutes
        # only the heavyweight adapter execution boundary in this unit test;
        # context loading, produce dispatch, failure handling and writes are real.
        code = ('import json,os,sys,unittest\n'
                'print(json.dumps({"NO_COLOR":os.environ.get("NO_COLOR"),'
                '"FORCE_COLOR":os.environ.get("FORCE_COLOR"),"CI":os.environ.get("CI")}))\n'
                'unittest.TextTestRunner().run(unittest.TestSuite([unittest.FunctionTestCase(lambda:None)]))\n'
                f'raise SystemExit({1 if fail else 0})\n')
        actual = subprocess.run([sys.executable, '-c', code], capture_output=True, text=True, check=False)
        from development_gates import test_counts
        count, skipped = test_counts(actual.stdout + actual.stderr)
        return json.loads(actual.stdout), {'exit_code': actual.returncode, 'test_count': count, 'skipped_count': skipped}

    def test_all_four_adapters_receive_plain_output_without_changing_outer_environment(self):
        observed = []
        def execute(root, change, run, check, gate, inputs):
            environment, receipt = self.child_environment()
            observed.append((check, environment))
            return receipt
        with patch.dict(os.environ, {'CI': 'true', 'FORCE_COLOR': '3'}):
            os.environ.pop('NO_COLOR', None)
            with patch('development_gates.execute', side_effect=execute):
                result = ci.produce(self.root)
            self.assertEqual(os.environ.get('FORCE_COLOR'), '3')
            self.assertNotIn('NO_COLOR', os.environ)
        self.assertEqual(result['adapters'], 4)
        self.assertEqual(result['test_count'], 4)
        self.assertEqual([check for check, _ in observed],
                         ['functional-component', 'affected-functional-tests', 'storage-migration', 'storage-runtime'])
        self.assertTrue(all(environment == {'NO_COLOR': '1', 'FORCE_COLOR': None, 'CI': 'true'}
                            for _, environment in observed))

    def test_failed_adapter_still_fails_and_restores_inherited_color_variables(self):
        observed = []
        def execute(*args):
            environment, receipt = self.child_environment(fail=True)
            observed.append(environment)
            return receipt
        with patch.dict(os.environ, {'CI': 'true', 'NO_COLOR': 'original', 'FORCE_COLOR': '2'}):
            with patch('development_gates.execute', side_effect=execute):
                with self.assertRaisesRegex(ValueError, 'adapter did not pass: functional-component'):
                    ci.produce(self.root)
            self.assertEqual(os.environ.get('NO_COLOR'), 'original')
            self.assertEqual(os.environ.get('FORCE_COLOR'), '2')
        self.assertEqual(observed, [{'NO_COLOR': '1', 'FORCE_COLOR': None, 'CI': 'true'}])
        self.assertFalse((self.root / '.build/quality/external.json').exists())

    def test_actual_vite_ci_color_breaks_url_parse_and_plain_output_restores_http_ready(self):
        directory = self.root / 'vite-input'; directory.mkdir()
        (directory / 'index.html').write_text('<!doctype html><title>Actual Vite readiness probe</title>')
        vite = ci.ROOT / 'apps/desktop/node_modules/vite/dist/node/index.js'
        code = r'''
const {createServer}=await import(process.argv[1]);
const path=await import('node:path');
const output=process.argv[2];let raw='';
const original=process.stdout.write.bind(process.stdout);
let server;
try {
  process.stdout.write=(chunk,encoding,callback)=>{raw+=String(chunk);if(typeof encoding==='function')encoding();else callback?.();return true;};
  server=await createServer({configFile:false,root:output,cacheDir:path.join(output,'cache'),server:{host:'127.0.0.1',port:0}});
  await server.listen();server.printUrls();
  process.stdout.write=original;
  const matched=raw.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]??null;
  const actualOrigin=server.resolvedUrls.local[0];
  const actualHttpOk=(await fetch(actualOrigin)).ok;
  const parsedHttpOk=matched===null?null:(await fetch(matched)).ok;
  console.log(JSON.stringify({raw,matched,actualOrigin,actualHttpOk,parsedHttpOk}));
} finally {process.stdout.write=original;await server?.close();}
'''
        def observe():
            actual = subprocess.run(['node', '--input-type=module', '-e', code, vite.as_uri(), str(directory)],
                                    capture_output=True, text=True, timeout=20, check=True)
            return json.loads(actual.stdout)
        with patch.dict(os.environ, {'CI': 'true', 'FORCE_COLOR': '3'}):
            os.environ.pop('NO_COLOR', None)
            colored = observe()
            with ci._plain_adapter_output():
                plain = observe()
        self.assertTrue(colored['actualHttpOk'])
        self.assertIn('\x1b[', colored['raw'])
        self.assertIsNone(colored['matched'])
        self.assertTrue(plain['actualHttpOk'])
        self.assertNotIn('\x1b[', plain['raw'])
        self.assertRegex(plain['matched'], r'^http://127\.0\.0\.1:\d+$')
        self.assertTrue(plain['parsedHttpOk'])


if __name__ == '__main__':
    unittest.main()
