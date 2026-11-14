"""Current-authority/archive guard mutations; never product verification."""
import copy
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from urllib.parse import unquote,urlsplit
import documentation_contract as d
import design_authority as authority

class DocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.paths={d.CONTRACT,d.SNAPSHOT,d.REGISTRY['archive_manifest'],d.LEGACY_ROOT_DESIGN,'README.md','AGENTS.md',*authority.current_approval_inputs(d.ROOT),*d.PROTECTED,*d.REGISTRY['documents'].values(),*(d.ARCHIVE+p for p in d.ARCHIVED_PATHS)}
        for p in d.REGISTRY['documents'].values():
            for raw in re.findall(r'\[[^\]]*\]\(([^)]+)\)',(d.ROOT/p).read_text()):
                u=urlsplit(raw.strip('<>'))
                if not u.scheme and not u.netloc and u.path:
                    target=((d.ROOT/p).parent/unquote(u.path)).resolve()
                    cls.paths.add(target.relative_to(d.ROOT).as_posix())
    def setUp(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup);self.root=Path(temp.name)
        for p in self.paths:
            source=d.ROOT/p;target=self.root/p
            if source.is_file():target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
            else: target.mkdir(parents=True,exist_ok=True)
        self.registry=json.loads((self.root/d.CONTRACT).read_text())
    def write(self,p,value):
        (self.root/p).write_text(value if isinstance(value,str) else json.dumps(value,ensure_ascii=False))
    def errors(self):return d.validate_contract(self.root)
    def test_current_contract_passes_without_product_claim(self):
        self.assertEqual([],self.errors());self.assertEqual(20,len(d.ARCHIVED_PATHS))
    def test_archive_or_second_authority_cannot_replace_current_role(self):
        for value in [d.ARCHIVE+'docs/product/first-product-scope.md',['docs/product/first-product-scope.md','docs/product/original/docs/01-prd.md']]:
            registry=copy.deepcopy(self.registry);registry['documents']['scope']=value;self.write(d.CONTRACT,registry)
            self.assertTrue(self.errors())
    def test_missing_duplicate_role_wrong_target_or_stale_entry_rejected(self):
        for field,value in [('entry','README.md'),('design_target','acceptance-v4'),('status','historical_only')]:
            registry=copy.deepcopy(self.registry);registry[field]=value;self.write(d.CONTRACT,registry);self.assertTrue(self.errors())
        registry=copy.deepcopy(self.registry);del registry['documents']['release'];self.write(d.CONTRACT,registry);self.assertTrue(self.errors())
    def test_duplicate_json_key_is_rejected(self):
        self.write(d.CONTRACT,'{"schema_version":1,"schema_version":1}')
        self.assertTrue(any('Duplicate JSON' in e for e in self.errors()))
    def test_archive_bytes_changed_even_if_manifest_rehashed_rejected(self):
        manifest=json.loads((self.root/d.REGISTRY['archive_manifest']).read_text());entry=manifest['entries'][0]
        p=self.root/entry['snapshot'];p.write_bytes(p.read_bytes()+b'changed historical text')
        entry['bytes']=p.stat().st_size;entry['sha256']=d.digest(p);self.write(d.REGISTRY['archive_manifest'],manifest)
        self.assertTrue(any('Historical snapshot' in e for e in self.errors()))
    def test_archive_missing_duplicate_or_current_status_rejected(self):
        original=json.loads((self.root/d.REGISTRY['archive_manifest']).read_text())
        for action in ('missing','duplicate','current'):
            m=copy.deepcopy(original)
            if action=='missing':m['entries'].pop()
            if action=='duplicate':m['entries'][-1]=copy.deepcopy(m['entries'][0])
            if action=='current':m['status']='current'
            self.write(d.REGISTRY['archive_manifest'],m);self.assertTrue(self.errors())
    def test_frozen_source_map_cannot_be_rewritten_to_new_history(self):
        snapshot=json.loads((self.root/d.SNAPSHOT).read_text());snapshot['files']['README.md']='a'*64;self.write(d.SNAPSHOT,snapshot)
        self.assertTrue(any('independently frozen' in e for e in self.errors()))
    def test_old_requirement_rows_cannot_be_lost_or_silently_revised(self):
        for p,prefix in [('docs/product/first-product-scope.md','FP'),('docs/product/model-configuration-prd.md','OT')]:
            text=(self.root/p).read_text();row=d._rows(text,prefix)[0];self.write(p,text.replace(row,row+' changed',1))
            self.assertTrue(any('stable requirement rows' in e for e in self.errors()));self.write(p,text)
    def test_ai_cfg_identity_cannot_disappear(self):
        p='docs/product/model-configuration-prd.md';text=(self.root/p).read_text();self.write(p,text.replace('### AI-CFG-01 ','### AI-CFG-99 ',1))
        self.assertTrue(any('16 stable AI' in e for e in self.errors()))
    def test_retained_machine_plan_or_approval_changed_rejected(self):
        for p in ('docs/development/current-product-test-coverage.json','docs/development/design-approval-20261019.json','LICENSE'):
            text=(self.root/p).read_bytes();(self.root/p).write_bytes(text+b' ');self.assertTrue(any('Frozen plan' in e for e in self.errors()));(self.root/p).write_bytes(text)
    def test_current_dated_patch_broken_link_and_empty_doc_rejected(self):
        p='docs/development/design-alignment-and-release.md';text=(self.root/p).read_text()
        for extra in ['\n## 2026-10-31 latest again\n',
                      '\n## 当前控件替换切片 · 2026-11-05\n',
                      '\n### 本轮技术接线 · 20261105\n',
                      '\n## 临时迁移 · 2026/11/05\n',
                      '\n[missing](missing-current.md)\n']:
            self.write(p,text+extra);self.assertTrue(self.errors())
        self.write(p,'');self.assertTrue(any('Empty current' in e for e in self.errors()))
    def test_duplicate_current_main_chapter_is_rejected(self):
        p='docs/development/iteration-governance.md';text=(self.root/p).read_text()
        chapter=re.search(r'^## (.+)$',text,re.M).group(1)
        self.write(p,text+'\n## '+chapter+' ##\nConflicting current rules\n')
        self.assertTrue(any('Duplicate current main chapter' in e for e in self.errors()))
    def test_document_examples_and_provenance_dates_are_not_current_patch_chapters(self):
        text='# Current document\n\nCaptured 2026-11-05.\n\n## Current rules\n\n```md\n## Example · 2026-11-05\n## Current rules\n```\n\n~~~md\n## Example · 20261105\n~~~\n\n## Verification\n'
        self.assertEqual([],d.current_heading_errors(text,'example.md'))
    def test_hub_root_routing_effective_approval_record_and_blocked_product_required(self):
        for p,old,new in [('README.md','docs/README.md','docs/product/README.md'),('docs/README.md','approved_by_user','pending_review'),('docs/README.md','design-approval-complete-20261031.json','design-approval-guided-20261031.json'),('docs/README.md','blocked','passed')]:
            text=(self.root/p).read_text();self.write(p,text.replace(old,new));self.assertTrue(self.errors());self.write(p,text)
    def test_hub_cannot_self_approve_without_pinned_current_record(self):
        self.assertIn('approved_by_user',(self.root/'docs/README.md').read_text())
        (self.root/authority.CURRENT_RECORD).unlink()
        self.assertTrue(any('source-version user approval invalid' in e for e in self.errors()))
    def test_page_coverage_role_is_mandatory_and_cannot_point_to_old_plan(self):
        registry=copy.deepcopy(self.registry);del registry['documents']['page_coverage'];self.write(d.CONTRACT,registry);self.assertTrue(self.errors())
        registry=copy.deepcopy(self.registry);registry['documents']['page_coverage']='docs/development/client-regression.md';self.write(d.CONTRACT,registry);self.assertTrue(self.errors())
    def test_complete_authority_composition_and_surface_routes_cannot_be_rebound(self):
        for field,value in [('design_composition_id','legacy-guided-only'),
                            ('design_authority',{'path':authority.RECORD,'sha256':authority.RECORD_SHA256}),
                            ('surface_entries',{'default':{'entry':authority.HISTORY_ROOT+'/index.html','query':{}}})]:
            with self.subTest(field=field):
                registry=copy.deepcopy(self.registry);registry[field]=value;self.write(d.CONTRACT,registry)
                self.assertTrue(self.errors())
    def test_history_product_and_plan_roles_are_required_current_documents(self):
        for role in ('history_product','history_plan'):
            with self.subTest(role=role):
                registry=copy.deepcopy(self.registry);del registry['documents'][role];self.write(d.CONTRACT,registry)
                self.assertTrue(self.errors())
                registry=copy.deepcopy(self.registry);registry['documents'][role]='docs/development/client-regression.md';self.write(d.CONTRACT,registry)
                self.assertTrue(self.errors())
    def test_linked_product_and_page_plans_are_current_guarded_roles(self):
        for role in ('ai_product','game_product','guided_plan','flat_plan'):
            with self.subTest(role=role):
                registry=copy.deepcopy(self.registry);del registry['documents'][role]
                self.write(d.CONTRACT,registry);self.assertTrue(self.errors())
                self.write(d.CONTRACT,self.registry)
                p=d.REGISTRY['documents'][role];text=(self.root/p).read_text()
                self.write(p,text+'\n## 当前补充 · 2026-11-05\n')
                self.assertTrue(any('Chronological append-only' in e for e in self.errors()))
                self.write(p,text)
    def test_existing_retired_root_prototype_is_not_a_current_settings_entry(self):
        p='docs/product/ai-reading-prd.md';text=(self.root/p).read_text()
        self.assertTrue((self.root/d.LEGACY_ROOT_DESIGN).is_file())
        self.write(p,text+'\n[当前设置](../design/first-product-v1/index.html?scenario=settings)\n')
        self.assertTrue(any('Retired root prototype' in e for e in self.errors()))
    def test_legacy_approval_still_valid_cannot_replace_current_complete_hub_decision(self):
        self.assertEqual([],authority.user_approval_gaps(self.root))
        (self.root/authority.CURRENT_RECORD).unlink()
        self.assertEqual([],authority.user_approval_gaps(self.root))
        self.assertTrue(any('complete source-version user approval invalid' in error for error in self.errors()))
    def test_path_symlink_and_archive_escape_rejected(self):
        p=self.root/d.ARCHIVE/'README.md';p.unlink();p.symlink_to(d.ROOT/'README.md');self.assertTrue(self.errors())

if __name__=='__main__':unittest.main()
