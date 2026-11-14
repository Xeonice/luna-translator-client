"""Independent malformed-contract/evidence tests, never product acceptance."""
import copy
import json
import base64
import hashlib
from pathlib import Path
import tempfile
import unittest
import flat_service_session as flat

class FlatServiceSessionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog=json.loads((flat.ROOT/flat.CATALOG).read_text())
    def catalog_error(self,mutate):
        value=copy.deepcopy(self.catalog);mutate(value);self.assertTrue(flat.validate_catalog(value))
    def test_current_static(self):self.assertEqual(flat.check(),[])
    def test_nonempty_fp_mapping(self):self.catalog_error(lambda d:d['flat_acceptance'][0].update(source_requirement_ids=[]))
    def test_plan_passed_rejected(self):
        body=(flat.ROOT/flat.PLAN).read_text();self.assertTrue(flat.validate_plan(body.replace('not_run |','passed |',1)))
    def test_scenario_zero_assertions_rejected(self):self.assertTrue(self.evidence(lambda d:d['checks'][0].update(assertions=0)))
    def test_catalog_valid(self):self.assertEqual(flat.validate_catalog(self.catalog),[])
    def test_catalog_shape(self):self.assertTrue(flat.validate_catalog([]))
    def test_flat_contract_required(self):self.catalog_error(lambda d:d.pop('flat_presentation_design'))
    def test_flat_contract_type(self):self.catalog_error(lambda d:d.update(flat_presentation_design=[]))
    def test_no_implicit_approval(self):self.catalog_error(lambda d:d['flat_presentation_design'].update(candidate_design_status='approved'))
    def test_wrong_candidate(self):self.catalog_error(lambda d:d['flat_presentation_design'].update(candidate_design='old.html'))
    def test_tabs_not_flat(self):self.catalog_error(lambda d:d['flat_presentation_design']['configuration_management'].update(layout='tabs'))
    def test_session_must_keep_three_regions(self):self.catalog_error(lambda d:d['flat_presentation_design']['current_session'].update(regions=['body_history']))
    def test_query_state_independent(self):self.catalog_error(lambda d:d['flat_presentation_design']['current_session'].update(independent_view_state=['query']))
    def test_product_promotion_rejected(self):self.catalog_error(lambda d:d['flat_presentation_design']['scope_boundaries'].update(product_acceptance='passed'))
    def test_bool_type_rejected(self):self.catalog_error(lambda d:d['flat_presentation_design']['scope_boundaries'].update(database_or_contract_changed=0))
    def test_protocol_removed_rejected(self):self.catalog_error(lambda d:d['flat_presentation_design']['scope_boundaries'].update(http_protocols=12))
    def test_fake_actions_rejected(self):self.catalog_error(lambda d:d['flat_presentation_design']['functional_audit'].update(no_empty_actions_or_fabricated_success=False))
    def test_fl_missing(self):self.catalog_error(lambda d:d['flat_acceptance'].pop())
    def test_fl_duplicate(self):self.catalog_error(lambda d:d['flat_acceptance'].__setitem__(1,copy.deepcopy(d['flat_acceptance'][0])))
    def test_fl_passed_product_rejected(self):self.catalog_error(lambda d:d['flat_acceptance'][0].update(status='passed'))
    def test_fl_future_layer_required(self):self.catalog_error(lambda d:d['flat_acceptance'][0].update(verification_layers=['design']))
    def test_fl_unknown_prd_rejected(self):self.catalog_error(lambda d:d['flat_acceptance'][0].update(source_requirement_ids=['FP-99']))
    def test_fl_unknown_reading_rejected(self):self.catalog_error(lambda d:d['flat_acceptance'][0].update(source_reading_requirement_ids=['RW-99']))
    def test_fl_bad_refs_shape(self):self.catalog_error(lambda d:d['flat_acceptance'][0].update(source_requirement_ids='FP-06'))
    def test_old_selection_contract_retained(self):self.catalog_error(lambda d:d['service_selection_design']['binding_contract_candidate'].update(save_changes_current_selection=True))
    def report_fixture(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);p=Path(tmp.name)
        png=base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZWkAAAAASUVORK5CYII=')
        (p/'a.png').write_bytes(png)
        value={'schema_version':1,'kind':'flat_service_session_design_verification','layer':'design','status':'passed','product_tests_executed':False,'assertions':10,'checks':[{'id':i,'status':'passed','assertions':1} for i in sorted(flat.FL_IDS)],'page_errors':[],'network_requests':[],'console_errors':[],'screenshots':[{'path':'a.png','sha256':hashlib.sha256(png).hexdigest(),'bytes':len(png),'width':1,'height':1}]}
        return p,value
    def evidence(self,mutate=None):
        p,value=self.report_fixture()
        if mutate:mutate(value)
        (p/'report.json').write_text(json.dumps(value));return flat._adapter_report(p)[1]
    def test_evidence_valid(self):self.assertEqual(self.evidence(),[])
    def test_evidence_missing_scenario(self):self.assertTrue(self.evidence(lambda d:d['checks'].pop()))
    def test_evidence_failed_check(self):self.assertTrue(self.evidence(lambda d:d['checks'][0].update(status='failed')))
    def test_evidence_network_rejected(self):self.assertTrue(self.evidence(lambda d:d['network_requests'].append('https://example.com')))
    def test_evidence_console_rejected(self):self.assertTrue(self.evidence(lambda d:d['console_errors'].append('error')))
    def test_evidence_product_rejected(self):self.assertTrue(self.evidence(lambda d:d.update(product_tests_executed=True)))
    def test_evidence_forged_png_rejected(self):self.assertTrue(self.evidence(lambda d:d['screenshots'][0].update(sha256='0'*64)))
    def test_evidence_zero_assertions_rejected(self):self.assertTrue(self.evidence(lambda d:d.update(assertions=0)))
    def test_evidence_png_path_escape(self):self.assertTrue(self.evidence(lambda d:d['screenshots'][0].update(path='../a.png')))
    def test_existing_destination_rejected(self):
        with tempfile.TemporaryDirectory(dir=flat.ROOT/'.build') as tmp:
            with self.assertRaises(ValueError):flat.execute(flat.ROOT,Path(tmp))
    def test_output_outside_build_rejected(self):
        with self.assertRaises(ValueError):flat._output_path(flat.ROOT,flat.ROOT/'reports/unsafe-output')
