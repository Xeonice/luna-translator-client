"""Guard-only receipts. These fixtures are never product PNG/runtime evidence."""
import copy
import hashlib
import json
import subprocess
import unittest
from product_execution import (BLOCKED_REFERENCE_STATUS, HISTORICAL_CAPTURE_BLOCKS, REFERENCE_CATALOGUE_V4, REQUIRED_V4_CAPTURES,
                               V4_REFERENCE_PRODUCER_INPUTS, load_reference_catalogue, validate_capture_report)
from design_source_versions import (MANIFEST_PATH, CATALOGUE_PATH, APPROVAL_PATH,
                                    VERSION_ID, PURPOSE, ALLOWED_RELOCATIONS,
                                    load_source_version, unique_json)
import test_product_execution as original_contract_tests


class ReviewedCaptureContractTests(unittest.TestCase):
    def setUp(self):
        self.fixture = original_contract_tests.ProductExecutionTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
        f = self.fixture
        original = copy.deepcopy(f.catalogue)
        f.catalogue = copy.deepcopy(original)
        f.catalogue.update(schemaVersion=4, states=[])
        f.rows, f.comparisons = [], []
        components = {'hud-toolbar-560', 'hud-toolbar-500', 'hud-ruby-only', 'hud-lexical-error',
                      'select-normal', 'select-long-list', 'preview-off', 'preview-live',
                      'preview-paused', 'preview-permission', 'preview-unavailable', 'preview-exited',
                      'session-extraction', 'session-hook-events'}
        for identity in sorted(REQUIRED_V4_CAPTURES):
            state = copy.deepcopy(original['states'][0])
            state.update(id=identity, surfaceKind='component' if identity in components else 'full-window')
            state.pop('underlay', None)
            conditions = dict(windowRole='main', viewport=dict(width=1040, height=720), theme='dark', transparency='glass')
            state['variants'] = [dict(variantId='top', expectedSize=[64,32], viewport=conditions['viewport'], deviceScaleFactor=1,
                                      selector='[data-guard-only-client]', expectedBounds=dict(x=0,y=0,width=64,height=32),
                                      scrollTop=[], requiredConditions=conditions)]
            reference = 'docs/design/first-product-v1/acceptance-v4/' + identity + '.png'
            f.freeze(reference, ('guard-only-not-png-' + identity).encode())
            sha = f.catalogue['frozenFiles'][reference]
            state['variants'][0].update(reference=reference, referenceSha256=sha)
            filenames = [identity + suffix for suffix in ['.baseline.png','.candidate.png','.diff.png']]
            for filename in filenames:
                (f.directory / filename).write_bytes((f.root / reference).read_bytes())
            f.rows.append(dict(id=identity, variantId='top', status='passed', reference=reference, referenceSha256=sha,
                               baselineCopy=filenames[0], candidate=filenames[1], diff=filenames[2], candidateSha256=sha, diffSha256=sha,
                               maxRatio=0, pixelmatchThreshold=.1, includeAA=True, differentPixels=0, ratio=0,
                               expectedSize=[64,32], actualSize=[64,32],
                               captureConditions=dict(viewport=conditions['viewport'], deviceScaleFactor=1, selector='[data-guard-only-client]',
                                                      bounds=dict(x=0,y=0,width=64,height=32),scrollTop=[],verifiedConditions=conditions)))
            f.comparisons.append(dict(id=identity,variantId='top',diffSha256=sha,differentPixels=0,ratio=0,expectedSize=[64,32],actualSize=[64,32]))
            f.catalogue['states'].append(state)
        approved = {}
        for number in range(16):
            source = f'docs/design/first-product-v1/guard-approved-{number}.js'
            f.freeze(source, ('guard-only-approved-source-' + str(number)).encode())
            approved[source] = f.catalogue['frozenFiles'][source]
        f.catalogue['approvedDesignSources'] = approved
        for source in V4_REFERENCE_PRODUCER_INPUTS:
            f.freeze(source, ('guard-only-independent-producer-' + source).encode())
        baseline_path = f.root / 'docs/development/iteration-baseline.json'
        baseline = json.loads(baseline_path.read_text())
        baseline['files'] += [dict(path=source, sha256=f.catalogue['frozenFiles'][source]) for source in sorted(V4_REFERENCE_PRODUCER_INPUTS)]
        baseline_path.write_text(json.dumps(baseline))
        authority = f.root / 'docs/development/design-approval-20261019.json'
        authority.write_text(json.dumps({'source_files_sha256':approved}))
        self.authority_sha = hashlib.sha256(authority.read_bytes()).hexdigest()
        self.version_relocations = {}
        self.save()

    def save(self):
        f = self.fixture
        path = f.root / REFERENCE_CATALOGUE_V4
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(f.catalogue))
        sha = hashlib.sha256(path.read_bytes()).hexdigest()
        baseline_path = f.root / 'docs/development/iteration-baseline.json'
        baseline = json.loads(baseline_path.read_text())
        baseline['files'] = [r for r in baseline['files'] if r['path'] not in {REFERENCE_CATALOGUE_V4,'docs/development/design-approval-20261019.json'}]
        baseline['files'] += [dict(path=REFERENCE_CATALOGUE_V4, sha256=sha), dict(path='docs/development/design-approval-20261019.json',sha256=self.authority_sha)]
        baseline_path.write_text(json.dumps(baseline))
        f.report.update(captures=f.rows, referenceCatalogueSha256=sha)
        # Explicit guard-only fixture registration. Runtime loaders never
        # generate or repin a missing map. Original negative tests intentionally
        # edit their catalogue; this fixture keeps that separate map input in
        # sync so each original authority/producer refusal is still exercised.
        entries = [dict(logical_path=source,
                        physical_path=self.version_relocations.get(source, source),
                        sha256=expected,
                        bytes=(f.root/self.version_relocations.get(source, source)).stat().st_size)
                   for source, expected in f.catalogue['frozenFiles'].items()]
        manifest = dict(schema_version=1, version_id=VERSION_ID, purpose=PURPOSE,
                        catalogue=dict(path=CATALOGUE_PATH, sha256=sha),
                        approval=dict(path=APPROVAL_PATH, sha256=self.authority_sha), entries=entries)
        self.pin_version_fixture(manifest)
        approved = json.loads((f.root/APPROVAL_PATH).read_text())['source_files_sha256']
        f.report['designSourceVersion'] = dict(
            manifestPath=MANIFEST_PATH, manifestSha256=hashlib.sha256((f.root/MANIFEST_PATH).read_bytes()).hexdigest(),
            versionId=VERSION_ID, catalogueSha256=sha, approvalSha256=self.authority_sha,
            resolutions=copy.deepcopy(entries),
            activeDesignSourceHashes={source:hashlib.sha256((f.root/source).read_bytes()).hexdigest() for source in approved})

    def pin_version_fixture(self, manifest=None, raw=None):
        """Only temporary guard fixtures may explicitly create a new authority."""
        f = self.fixture
        path = f.root/MANIFEST_PATH
        path.parent.mkdir(parents=True, exist_ok=True)
        data = raw if raw is not None else json.dumps(manifest).encode()
        path.write_bytes(data)
        baseline_path = f.root/'docs/development/iteration-baseline.json'
        baseline = json.loads(baseline_path.read_text())
        baseline['files'] = [row for row in baseline['files'] if row['path'] != MANIFEST_PATH]
        baseline['files'].append(dict(path=MANIFEST_PATH, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data)))
        baseline_path.write_text(json.dumps(baseline))

    def version_fixture(self):
        return json.loads((self.fixture.root/MANIFEST_PATH).read_text())

    def install_historical_alias(self, logical=None):
        f = self.fixture
        logical = logical or next(iter(ALLOWED_RELOCATIONS))
        physical = ALLOWED_RELOCATIONS[logical]
        f.freeze(logical, b'guard-only original approved design bytes')
        f.freeze(physical, (f.root/logical).read_bytes())
        approved = dict(f.catalogue['approvedDesignSources'])
        approved.pop(next(iter(approved)))
        approved[logical] = f.catalogue['frozenFiles'][logical]
        f.catalogue['approvedDesignSources'] = approved
        authority = f.root/APPROVAL_PATH
        authority.write_text(json.dumps({'source_files_sha256':approved}))
        self.authority_sha = hashlib.sha256(authority.read_bytes()).hexdigest()
        self.version_relocations[logical] = physical
        (f.root/logical).write_bytes(b'guard-only later authorized active design')
        self.save()
        return logical, physical

    def test_all_original_windows_new_windows_and_components_required(self):
        f = self.fixture
        states, _ = load_reference_catalogue(f.root, 'product-ui-capture-v4')
        self.assertEqual(set(states), REQUIRED_V4_CAPTURES)
        errors, _, _ = validate_capture_report(f.root,f.directory,f.report,f.comparisons,0,'product-ui-capture-v4')
        self.assertEqual(errors, [])
        for missing in ['reading-hud','library-populated','session-current-hook-events','preview-live','settings-shortcuts-conflict']:
            saved = f.catalogue['states']
            f.catalogue['states'] = [r for r in saved if r['id'] != missing]
            self.save()
            with self.assertRaisesRegex(ValueError,'complete independently required'):
                load_reference_catalogue(f.root,'product-ui-capture-v4')
            f.catalogue['states'] = saved

    def install_blocked_obligations(self):
        f = self.fixture
        for index, row in enumerate(f.rows):
            code = HISTORICAL_CAPTURE_BLOCKS.get(row['id'])
            if code:
                f.rows[index] = {key: row[key] for key in ('id', 'variantId', 'reference', 'referenceSha256', 'expectedSize')}
                f.rows[index].update(status=BLOCKED_REFERENCE_STATUS, blocked=dict(
                    code=code, replacementReference=None, reason='Guard-only retired premise; replacement reference pending.'))
        f.comparisons = [row for row in f.comparisons if row['id'] not in HISTORICAL_CAPTURE_BLOCKS]
        f.report.update(status='failed', visualStatus='failed')
        self.save()

    def test_retired_obligations_remain_in_inventory_and_cannot_pass(self):
        self.install_blocked_obligations()
        f = self.fixture
        errors, behavior, visual = validate_capture_report(f.root, f.directory, f.report, f.comparisons, 1, 'product-ui-capture-v4')
        self.assertEqual(set(row['id'] for row in f.rows), REQUIRED_V4_CAPTURES)
        self.assertEqual((behavior, visual), ('passed', 'failed'))
        self.assertEqual(len([error for error in errors if BLOCKED_REFERENCE_STATUS in error]), 5)
        self.assertFalse(any('inventory' in error or 'contradict' in error for error in errors), errors)
        f.report.update(status='passed', visualStatus='passed')
        errors, _, _ = validate_capture_report(f.root, f.directory, f.report, f.comparisons, 0, 'product-ui-capture-v4')
        self.assertTrue(any('contradicts' in error for error in errors))

    def test_unknown_partial_missing_or_forged_blocked_obligations_fail(self):
        self.install_blocked_obligations()
        f = self.fixture
        original = copy.deepcopy(f.rows)
        blocked = next(row for row in f.rows if row['id'] in HISTORICAL_CAPTURE_BLOCKS)
        for mutation in ('passed', 'missing', 'fake_pixels', 'wrong_code', 'unknown_state'):
            with self.subTest(mutation=mutation):
                f.rows[:] = copy.deepcopy(original)
                row = next(row for row in f.rows if row['id'] == blocked['id'])
                if mutation == 'passed':
                    row['status'] = 'passed'
                elif mutation == 'missing':
                    f.rows.remove(row)
                elif mutation == 'fake_pixels':
                    row.update(actualSize=row['expectedSize'], differentPixels=0, ratio=0)
                elif mutation == 'wrong_code':
                    row['blocked']['code'] = 'pretend_current_adapter_missing'
                else:
                    live = next(row for row in f.rows if row['id'] == 'preview-live')
                    live.clear()
                    live.update(copy.deepcopy(blocked), id='preview-live')
                self.save()
                errors, _, visual = validate_capture_report(f.root, f.directory, f.report, f.comparisons, 1, 'product-ui-capture-v4')
                self.assertEqual(visual, 'failed')
                self.assertTrue(any(word in error for error in errors for word in ('inventory', 'forged', 'unregistered historical', 'contradicts', 'contradict')), errors)

    def test_visible_query_receipt_must_match_every_independent_field(self):
        f = self.fixture
        state = next(row for row in f.catalogue['states'] if row['id'] == 'session-hook-events')
        query = dict(sessionId='guard-session', dataset='hook_events', filter=dict(search=None, sourceId=None, level=None, stage=None), order='newest_first', pageSize=50, cursor=None)
        state['dataState']['componentInputs'] = dict(supplement=dict(query=query))
        row = next(row for row in f.rows if row['id'] == state['id'])
        row['captureConditions']['sessionRecordQuery'] = copy.deepcopy(query)
        self.save()
        errors, _, _ = validate_capture_report(f.root, f.directory, f.report, f.comparisons, 0, 'product-ui-capture-v4')
        self.assertEqual(errors, [])
        for field, value in [('sessionId','another'), ('dataset','extraction'), ('order','oldest_first'), ('pageSize',49), ('cursor','opaque'), ('search','q'), ('sourceId','another'), ('level','warning'), ('stage','diagnostics')]:
            with self.subTest(field=field):
                actual = copy.deepcopy(query)
                actual['filter' if field in query['filter'] else field] = ({**actual['filter'], field:value} if field in query['filter'] else value)
                row['captureConditions']['sessionRecordQuery'] = actual
                errors, _, visual = validate_capture_report(f.root, f.directory, f.report, f.comparisons, 0, 'product-ui-capture-v4')
                self.assertTrue(any('session/dataset/filter/order/pageSize/cursor' in error for error in errors))
                self.assertEqual(visual, 'failed')

    def test_browser_capture_helpers_reject_wrong_queries_and_ambiguous_preview_owners(self):
        script = r"""
import assert from 'node:assert/strict';
import {assertPreviewOwner} from './apps/desktop/tests/visual/capture-reviewed-states.mjs';
import {assertReviewedSessionQuery, captureCatalogueState, historicalCaptureBlock, HISTORICAL_CAPTURE_BLOCKS, scrollConditionDifferences} from './apps/desktop/tests/visual/capture-catalogue.mjs';
const owner={consumerId:'chosen',leaseId:'lease',generation:'3'};
const projection={...owner,state:'live'};
const makePort=()=>({previewProjections:new Map([['lease',projection],['last-unrelated',{consumerId:'unrelated',leaseId:'last-unrelated',generation:'99',state:'live'}]]),previewListeners:new Map([['subscriber',{projection}]])});
globalThis.window={uiHarness:makePort()};
assert.deepEqual(assertPreviewOwner(owner),projection);
for(const key of ['consumerId','leaseId','generation'])assert.throws(()=>assertPreviewOwner({...owner,[key]:'wrong'}),/owner/);
assert.throws(()=>assertPreviewOwner({consumerId:owner.consumerId}),/explicit/);
window.uiHarness.previewListeners.set('duplicate',{projection});
assert.throws(()=>assertPreviewOwner(owner),/one exact/);
window.uiHarness=makePort();
window.uiHarness.previewProjections.set('duplicate',{...projection,leaseId:'duplicate'});
assert.throws(()=>assertPreviewOwner(owner),/one exact/);
window.uiHarness=makePort();window.uiHarness.previewListeners.clear();
assert.throws(()=>assertPreviewOwner(owner),/one exact/);
const query={sessionId:'session',dataset:'hook_events',filter:{search:null,sourceId:null,level:null,stage:null},order:'newest_first',pageSize:50,cursor:null};
assert.deepEqual(assertReviewedSessionQuery(query,query),query);
for(const key of Object.keys(query))assert.throws(()=>assertReviewedSessionQuery({...query,[key]:'wrong'},query),/independent fixture/);
for(const key of Object.keys(query.filter))assert.throws(()=>assertReviewedSessionQuery({...query,filter:{...query.filter,[key]:'wrong'}},query),/independent fixture/);
const scroll=[{selector:'.scroller',top:0,clientHeight:100,scrollHeight:300}];
for(const field of Object.keys(scroll[0])){
 const actual=structuredClone(scroll);actual[0][field]=field==='selector'?'.another':101;
 const differences=scrollConditionDifferences(scroll,actual);assert.equal(differences.length,1);assert.equal(differences[0].field,field);
}
assert.notEqual(scrollConditionDifferences(scroll,[]).length,0);
assert.notEqual(scrollConditionDifferences(scroll,[{...scroll[0],extra:1}]).length,0);
for(const [id,code] of Object.entries(HISTORICAL_CAPTURE_BLOCKS)){
 const variants=['top','bottom'].map(variantId=>({variantId,reference:id+'.'+variantId+'.png',referenceSha256:'a'.repeat(64),expectedSize:[1,1]}));
 const catalogue={schemaVersion:4,states:[{id,variants}],file:'guard-only',sha256:'b'.repeat(64)};
 const row=await captureCatalogueState({page:new Proxy({},{get(){throw Error('retired state must never open a page');}}),id,catalogue});
 assert.equal(row.status,'blocked_pending_replacement_reference');assert.equal(row.blocked.code,code);
 assert.equal(row.variants.length,1);assert.equal(row.variants[0].reference,variants[1].reference);
 for(const child of [row,...row.variants])for(const key of ['actualSize','candidate','differentPixels','ratio'])assert.equal(Object.hasOwn(child,key),false);
}
assert.equal(historicalCaptureBlock({schemaVersion:4},'target-review'),null);
assert.equal(historicalCaptureBlock({schemaVersion:4},'preview-live'),null);
assert.equal(historicalCaptureBlock({schemaVersion:3},'preview-off'),null);
"""
        result = subprocess.run(['node', '--input-type=module', '-e', script], cwd=original_contract_tests.ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_independent_png_comparator_preserves_uncaptured_blocked_facts(self):
        self.install_blocked_obligations()
        f = self.fixture
        report = f.directory / 'blocked-comparison.json'
        blocked = [row for row in f.rows if row['status'] == BLOCKED_REFERENCE_STATUS]
        report.write_text(json.dumps(dict(captures=blocked)))
        command = ['node', 'scripts/contracts/compare_product_capture.mjs', str(report)]
        result = subprocess.run(command, cwd=original_contract_tests.ROOT, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), [])
        # Changing the status cannot turn missing evidence into equality. Every
        # non-blocked row still has to decode its actual baseline and candidate.
        blocked[0]['status'] = 'failed'
        report.write_text(json.dumps(dict(captures=blocked)))
        result = subprocess.run(command, cwd=original_contract_tests.ROOT, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)

    def test_component_cannot_stand_in_for_complete_window(self):
        f = self.fixture
        next(r for r in f.catalogue['states'] if r['id']=='session-current-body')['surfaceKind']='component'
        self.save()
        with self.assertRaisesRegex(ValueError,'remain distinct'):
            load_reference_catalogue(f.root,'product-ui-capture-v4')

    def test_external_producer_requires_separate_authority_pin(self):
        f = self.fixture
        source = next(iter(V4_REFERENCE_PRODUCER_INPUTS))
        baseline_path = f.root / 'docs/development/iteration-baseline.json'
        baseline = json.loads(baseline_path.read_text())
        baseline['files'] = [row for row in baseline['files'] if row['path'] != source]
        baseline_path.write_text(json.dumps(baseline))
        with self.assertRaisesRegex(ValueError, 'External reference producer'):
            load_reference_catalogue(f.root, 'product-ui-capture-v4')

    def test_catalogue_repin_cannot_self_authorize_producer_change(self):
        f = self.fixture
        source = next(iter(V4_REFERENCE_PRODUCER_INPUTS))
        f.freeze(source, b'changed-without-independent-producer-review')
        self.save()
        with self.assertRaisesRegex(ValueError, 'External reference producer'):
            load_reference_catalogue(f.root, 'product-ui-capture-v4')

    def test_producer_closure_cannot_omit_input_or_expand_to_product_source(self):
        f = self.fixture
        source = next(iter(V4_REFERENCE_PRODUCER_INPUTS))
        saved = f.catalogue['frozenFiles'].pop(source)
        self.save()
        with self.assertRaisesRegex(ValueError, 'producer closure is incomplete'):
            load_reference_catalogue(f.root, 'product-ui-capture-v4')
        f.catalogue['frozenFiles'][source] = saved
        f.freeze('apps/desktop/src/arbitrary-product-implementation.ts', b'not-an-independent-design-input')
        self.save()
        with self.assertRaisesRegex(ValueError, 'Frozen design input/hash differs'):
            load_reference_catalogue(f.root, 'product-ui-capture-v4')

    def test_fresh_catalogue_cannot_adopt_unapproved_design_bytes(self):
        f = self.fixture
        source = next(iter(f.catalogue['approvedDesignSources']))
        (f.root/source).write_bytes(b'changed-without-original-design-review')
        sha = hashlib.sha256((f.root/source).read_bytes()).hexdigest()
        f.catalogue['approvedDesignSources'][source]=sha
        f.catalogue['frozenFiles'][source]=sha
        self.save()
        with self.assertRaisesRegex(ValueError,'sixteen current approved'):
            load_reference_catalogue(f.root,'product-ui-capture-v4')

    def test_design_approval_and_zero_threshold_do_not_self_authorize(self):
        f = self.fixture
        f.catalogue['pixelmatch']['maxRatio']=.01
        self.save()
        with self.assertRaisesRegex(ValueError,'independently registered'):
            load_reference_catalogue(f.root,'product-ui-capture-v4')
        f.catalogue['pixelmatch']['maxRatio']=0
        self.save()
        (f.root/'docs/development/design-approval-20261019.json').write_text('{}')
        with self.assertRaisesRegex(ValueError,'independent authority baseline'):
            load_reference_catalogue(f.root,'product-ui-capture-v4')

    def test_historical_alias_preserves_logical_identity_and_distinct_active_source(self):
        logical, physical = self.install_historical_alias()
        f = self.fixture
        version = load_source_version(f.root)
        self.assertEqual(version.resolve(logical), f.root.resolve()/physical)
        self.assertEqual(version.resolve(physical), f.root.resolve()/physical)
        row = next(r for r in version.receipt['resolutions'] if r['logical_path'] == logical)
        self.assertEqual(row['sha256'], f.catalogue['frozenFiles'][logical])
        self.assertNotEqual(row['sha256'], version.receipt['activeDesignSourceHashes'][logical])
        states, _ = load_reference_catalogue(f.root, 'product-ui-capture-v4')
        self.assertEqual(set(states), REQUIRED_V4_CAPTURES)
        errors, _, _ = validate_capture_report(f.root,f.directory,f.report,f.comparisons,0,'product-ui-capture-v4')
        self.assertEqual(errors, [])

    def test_source_version_requires_exact_separate_single_authority_pin(self):
        f = self.fixture
        baseline_path = f.root/'docs/development/iteration-baseline.json'
        saved = json.loads(baseline_path.read_text())
        for kind in ['absent', 'duplicate', 'wrong_hash']:
            with self.subTest(kind=kind):
                baseline = copy.deepcopy(saved)
                pin = next(r for r in baseline['files'] if r['path'] == MANIFEST_PATH)
                if kind == 'absent': baseline['files'].remove(pin)
                elif kind == 'duplicate': baseline['files'].append(copy.deepcopy(pin))
                else: pin['sha256'] = '0'*64
                baseline_path.write_text(json.dumps(baseline))
                with self.assertRaisesRegex(ValueError, 'independent authority pin'):
                    load_source_version(f.root)
        baseline_path.write_text(json.dumps(saved))
        (f.root/MANIFEST_PATH).unlink()
        with self.assertRaises(ValueError): load_source_version(f.root)

    def test_source_version_contract_rejects_wrong_versions_and_self_authorized_options(self):
        original = self.version_fixture()
        for key, value in [('version_id','unreviewed-version'),('schema_version',True),
                           ('purpose','approve-current-design'),('catalogue',dict(path=CATALOGUE_PATH,sha256='0'*64)),
                           ('approval',dict(path=APPROVAL_PATH,sha256='0'*64)),('fallback',True)]:
            with self.subTest(key=key):
                manifest = copy.deepcopy(original); manifest[key] = value
                self.pin_version_fixture(manifest)
                with self.assertRaisesRegex(ValueError, 'manifest version/catalogue/approval'):
                    load_source_version(self.fixture.root)

    def test_source_version_domain_missing_extra_duplicate_and_wrong_hash_are_rejected(self):
        original = self.version_fixture()
        for kind in ['missing', 'extra', 'duplicate', 'wrong_hash']:
            with self.subTest(kind=kind):
                manifest = copy.deepcopy(original)
                if kind == 'missing': manifest['entries'].pop()
                elif kind == 'extra': manifest['entries'].append(dict(manifest['entries'][0], logical_path='docs/design/first-product-v1/unregistered.js'))
                elif kind == 'duplicate': manifest['entries'][-1] = copy.deepcopy(manifest['entries'][0])
                else: manifest['entries'][0]['sha256'] = '0'*64
                self.pin_version_fixture(manifest)
                with self.assertRaises(ValueError): load_source_version(self.fixture.root)

    def test_source_version_cannot_relocate_png_producer_or_adapter(self):
        f = self.fixture
        for logical in [f.rows[0]['reference'], next(iter(V4_REFERENCE_PRODUCER_INPUTS)), f.adapter_manifest]:
            with self.subTest(logical=logical):
                physical = 'docs/design/first-product-v1/acceptance-v4/guard-only-copy.bin'
                f.freeze(physical, (f.root/logical).read_bytes()); self.save()
                manifest = self.version_fixture()
                row = next(r for r in manifest['entries'] if r['logical_path'] == logical)
                row['physical_path'] = physical
                self.pin_version_fixture(manifest)
                with self.assertRaisesRegex(ValueError, 'Unapproved source-version relocation'):
                    load_source_version(f.root)

    def test_source_version_rejects_unsafe_and_unlisted_physical_paths(self):
        original = self.version_fixture()
        for path in ['../escape', '/tmp/source.js', 'https://invalid/source.js', r'C:\\source.js',
                     'docs//source.js', 'docs/./source.js', '.build/unlisted-backup.js']:
            with self.subTest(path=path):
                manifest = copy.deepcopy(original); manifest['entries'][0]['physical_path'] = path
                if path == '.build/unlisted-backup.js':
                    source = original['entries'][0]['physical_path']
                    backup = self.fixture.root/path; backup.parent.mkdir(parents=True, exist_ok=True)
                    backup.write_bytes((self.fixture.root/source).read_bytes())
                self.pin_version_fixture(manifest)
                with self.assertRaises(ValueError): load_source_version(self.fixture.root)

    def test_historical_origin_symlink_tamper_missing_and_wrong_size_do_not_fallback(self):
        logical, physical = self.install_historical_alias(); f = self.fixture
        origin = f.root/physical; original_bytes = origin.read_bytes(); manifest = self.version_fixture()
        # Even valid-looking live bytes do not authorize a fallback.
        (f.root/logical).write_bytes(original_bytes)
        origin.write_bytes(b'corrupt-origin')
        with self.assertRaisesRegex(ValueError, 'physical bytes differ'): load_source_version(f.root)
        origin.unlink()
        with self.assertRaises(ValueError): load_source_version(f.root)
        alternate = origin.parent/'guard-alternate.js'; alternate.write_bytes(original_bytes)
        origin.symlink_to(alternate.name)
        with self.assertRaisesRegex(ValueError, 'symlink'): load_source_version(f.root)
        origin.unlink(); origin.write_bytes(original_bytes)
        row = next(r for r in manifest['entries'] if r['logical_path'] == logical); row['bytes'] += 1
        self.pin_version_fixture(manifest)
        with self.assertRaisesRegex(ValueError, 'physical bytes differ'): load_source_version(f.root)

    def test_identity_tamper_is_rejected_without_searching_for_other_bytes(self):
        f = self.fixture
        source = next(iter(f.catalogue['approvedDesignSources']))
        backup = f.root/'.build'/source.rsplit('/',1)[-1]
        backup.parent.mkdir(parents=True, exist_ok=True); backup.write_bytes((f.root/source).read_bytes())
        (f.root/source).write_bytes(b'changed-identity-input')
        with self.assertRaisesRegex(ValueError, 'physical bytes differ'): load_source_version(f.root)

    def test_mapping_chain_from_an_origin_is_never_authorized(self):
        logical, physical = self.install_historical_alias(); manifest = self.version_fixture()
        next(r for r in manifest['entries'] if r['logical_path'] == physical)['physical_path'] = logical
        self.pin_version_fixture(manifest)
        with self.assertRaisesRegex(ValueError, 'Unapproved source-version relocation'):
            load_source_version(self.fixture.root)

    def test_source_version_duplicate_json_keys_are_not_silently_accepted(self):
        original = (self.fixture.root/MANIFEST_PATH).read_text()
        for raw in ['{"purpose":"first",'+original[1:], original.replace('"physical_path":', '"physical_path":"ignored", "physical_path":',1),
                    original.replace('"version_id":', '"version_\\u0069d":"ignored", "version_id":',1)]:
            with self.subTest(raw=raw[:90]):
                self.pin_version_fixture(raw=raw.encode())
                with self.assertRaisesRegex(ValueError, 'Duplicate source-version JSON key'):
                    load_source_version(self.fixture.root)
        with self.assertRaises(ValueError): unique_json(b'\xff{}')

    def test_source_version_receipt_missing_or_forged_is_rejected(self):
        f = self.fixture; saved = copy.deepcopy(f.report['designSourceVersion'])
        for kind in ['missing', 'manifest_hash', 'physical_trace', 'active_hash']:
            with self.subTest(kind=kind):
                f.report['designSourceVersion'] = copy.deepcopy(saved)
                if kind == 'missing': f.report.pop('designSourceVersion')
                elif kind == 'manifest_hash': f.report['designSourceVersion']['manifestSha256'] = '0'*64
                elif kind == 'physical_trace': f.report['designSourceVersion']['resolutions'][0]['physical_path'] = '.build/forged.js'
                else:
                    key = next(iter(f.report['designSourceVersion']['activeDesignSourceHashes']))
                    f.report['designSourceVersion']['activeDesignSourceHashes'][key] = '0'*64
                errors, _, _ = validate_capture_report(f.root,f.directory,f.report,f.comparisons,0,'product-ui-capture-v4')
                self.assertTrue(any('source-version receipt' in error for error in errors))

    def assert_exact_historical_alias(self, name):
        """A real temporary authority fixture, never current design approval."""
        logical, physical = self.install_historical_alias('docs/design/first-product-v1/' + name)
        f = self.fixture
        version = load_source_version(f.root)
        self.assertEqual(version.resolve(logical), f.root.resolve()/physical)
        self.assertEqual(version.resolve(physical), f.root.resolve()/physical)
        raw = version.resolve(logical).read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), f.catalogue['frozenFiles'][logical])
        self.assertNotEqual(hashlib.sha256((f.root/logical).read_bytes()).hexdigest(), f.catalogue['frozenFiles'][logical])
        row = next(row for row in version.receipt['resolutions'] if row['logical_path'] == logical)
        self.assertEqual(row['bytes'], len(raw))
        self.assertEqual(row['sha256'], f.catalogue['frozenFiles'][logical])
        self.assertEqual(set(version.paths), set(f.catalogue['frozenFiles']))
        self.assertEqual(len(f.catalogue['approvedDesignSources']), 16)
        return version

    def test_compact_design_data_alias_keeps_exact_historical_bytes(self):
        self.assert_exact_historical_alias('data.js')

    def test_compact_design_index_alias_keeps_exact_historical_bytes(self):
        self.assert_exact_historical_alias('index.html')

    def test_compact_design_coverage_alias_keeps_exact_historical_bytes(self):
        self.assert_exact_historical_alias('coverage.csv')

    def test_compact_design_integration_alias_keeps_exact_historical_bytes(self):
        self.assert_exact_historical_alias('integration.md')

    def test_compact_design_manifest_alias_keeps_exact_historical_bytes(self):
        self.assert_exact_historical_alias('manifest.json')

    def test_compact_design_style_guide_alias_keeps_exact_historical_bytes(self):
        self.assert_exact_historical_alias('style-guide.md')

    def test_all_eleven_exact_aliases_and_original_five_remain_compatible(self):
        original_five = ('app.js', 'game-library.js', 'review-gaps.css', 'review-gaps.js', 'README.md')
        for name in original_five:
            self.assert_exact_historical_alias(name)
        self.assertEqual(len(self.version_relocations), 5)
        for name in ('data.js', 'index.html', 'coverage.csv', 'integration.md', 'manifest.json', 'style-guide.md'):
            self.assert_exact_historical_alias(name)
        version = load_source_version(self.fixture.root)
        self.assertEqual(len(self.version_relocations), 11)
        self.assertEqual(dict(self.version_relocations), dict(ALLOWED_RELOCATIONS))
        self.assertEqual(sum(row['logical_path'] != row['physical_path'] for row in version.receipt['resolutions']), 11)
        errors, _, _ = validate_capture_report(self.fixture.root,self.fixture.directory,self.fixture.report,self.fixture.comparisons,0,'product-ui-capture-v4')
        self.assertEqual(errors, [])

    def test_twelfth_unapproved_alias_is_refused_even_with_frozen_same_bytes(self):
        f = self.fixture
        logical = 'docs/design/first-product-v1/controls.js'
        physical = 'docs/design/first-product-v1/acceptance-v4/design-origin/controls.js'
        for name in ALLOWED_RELOCATIONS:
            self.install_historical_alias(name)
        f.freeze(logical, b'guard-only unapproved twelfth exact copy')
        f.freeze(physical, (f.root/logical).read_bytes())
        self.version_relocations[logical] = physical
        self.save()
        self.assertEqual(len(self.version_relocations), 12)
        with self.assertRaisesRegex(ValueError, 'Unapproved source-version relocation'):
            load_source_version(f.root)

    def test_six_new_aliases_still_refuse_wrong_bytes_size_chains_and_escape(self):
        for name in ('data.js', 'index.html', 'coverage.csv', 'integration.md', 'manifest.json', 'style-guide.md'):
            logical, physical = self.install_historical_alias('docs/design/first-product-v1/' + name)
            f = self.fixture
            original = self.version_fixture()
            origin = f.root/physical
            original_bytes, active_bytes = origin.read_bytes(), (f.root/logical).read_bytes()
            # A valid active file is deliberately present; no fallback is allowed.
            (f.root/logical).write_bytes(original_bytes)
            with self.subTest(name=name, negative='wrong_bytes'):
                origin.write_bytes(b'x'*len(original_bytes))
                with self.assertRaisesRegex(ValueError, 'physical bytes differ'):
                    load_source_version(f.root)
                origin.write_bytes(original_bytes)
            for negative in ('wrong_size', 'chain', 'escape'):
                with self.subTest(name=name, negative=negative):
                    manifest = copy.deepcopy(original)
                    if negative == 'wrong_size':
                        next(row for row in manifest['entries'] if row['logical_path'] == logical)['bytes'] += 1
                    elif negative == 'chain':
                        next(row for row in manifest['entries'] if row['logical_path'] == physical)['physical_path'] = logical
                    else:
                        next(row for row in manifest['entries'] if row['logical_path'] == logical)['physical_path'] = '../escape.js'
                    self.pin_version_fixture(manifest)
                    with self.assertRaises(ValueError):
                        load_source_version(f.root)
            self.pin_version_fixture(original)
            (f.root/logical).write_bytes(active_bytes)
