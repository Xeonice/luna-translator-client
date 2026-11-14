"""Negative regressions for omitted surfaces/states and fabricated UI evidence.

These test the guard only. They do not execute or accept the product.
"""
import ast
import copy
import json
import hashlib
import re
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch
import all_page_design_coverage as c


class AllPageDesignCoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        build=c.ROOT/'.build'; build.mkdir(exist_ok=True)
        cls.workspace=tempfile.TemporaryDirectory(prefix='all-page-guard-',dir=build)
        cls.base=Path(cls.workspace.name)/'baseline'; cls.base.mkdir()
        shutil.copytree(c.ROOT/'apps/desktop/src',cls.base/'apps/desktop/src')
        files=[c.CONTRACT,c.MODEL_CONFIG_LINEAGE,'docs/architecture/engineering-components.csv','docs/product/first-product-scope.md','docs/product/model-configuration-prd.md',
               'docs/product/game-library-window-acceptance.csv','docs/product/reading-window-session-acceptance.csv','docs/product/model-configuration-acceptance.csv',*c.DESIGN_FIXTURE_SHA256,*c.authority.current_approval_inputs(c.ROOT)]
        from current_general_visual_authority import source_inputs
        files.extend(source_inputs(c.ROOT))
        files.append('apps/desktop/tests/fixtures/general-visual/legacy-general-descriptors.json')
        for relative in files:
            destination=cls.base/relative; destination.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(c.ROOT/relative,destination)
        cls.document=c._read(cls.base/c.CONTRACT)
        cls.ast=c.inventory(cls.base)

    @classmethod
    def tearDownClass(cls): cls.workspace.cleanup()

    def setUp(self):
        self.directory=tempfile.TemporaryDirectory(prefix='case-',dir=Path(self.workspace.name))
        self.root=Path(self.directory.name)/'repo'; shutil.copytree(self.base,self.root)

    def tearDown(self): self.directory.cleanup()

    def mutate(self,fn):
        data=copy.deepcopy(self.document); fn(data)
        (self.root/c.CONTRACT).write_text(json.dumps(data,ensure_ascii=False))

    def assertRejected(self): self.assertTrue(c.validate_contract(self.root),'mutated input must be rejected')

    def test_general_only_migration_preserves_other_pages_global_ids_and_layers(self):
        from current_general_visual_authority import STATE_IDS,expected_mapping
        data=copy.deepcopy(self.document)
        general=next(p for p in data['pages'] if p['surface_id']=='settings:general')
        self.assertEqual([],c.validate_general_mapping(self.root))
        self.assertEqual(expected_mapping(self.root),general['source_supplement'])
        self.assertEqual(set(STATE_IDS),{v['id'] for v in general['states']})
        lineage=c.model_config_lineage(self.root)
        original_ids={s['id'] for s in lineage['original_all_page_states']}
        canonical=lambda value:hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
        # The frozen pre-migration document identity remains auditable. Current
        # AST production dependencies legitimately differ; original state/source
        # semantics are checked one by one, not replaced with a new global hash.
        self.assertEqual('7045fc9e833052b2b2d4040bccba85698f0a1ad56a6a127c95ed872a2f393e6e',lineage['original_non_general_document_sha256'])
        self.assertEqual([],c.validate_model_config_lineage(self.root,data))
        ids=[v['id'] for p in data['pages'] for v in p['states'] if v['id'] in original_ids]
        layers={v['id']:v['required_layers'] for p in data['pages'] for v in p['states'] if v['id'] in original_ids}
        self.assertEqual(856,len(ids));self.assertEqual('081be586ee45706ede658b19d167b8da71f8a2eaaa89da6b8d141eda5e5d5351',canonical(ids))
        self.assertEqual('b8b95174bef60d8a8f7927eff123a5d58f24ca4a8f98fcabab03da61cc84fa06',canonical(layers))
        original_applicable={s['id'] for s in lineage['original_all_page_states'] if s['applicable']}
        self.assertEqual(780,len(original_applicable));self.assertLess(original_applicable,c.STATE_IDS)
        self.assertTrue(all(v['execution']=={'real_react':'not_run','packaged_native':'not_run'} and v['product_status']=='blocked' for v in general['states']))
        for mutate in ('old_source','missing_state','fake_reference','alternate_candidate','dependency'):
            altered=copy.deepcopy(data);page=next(p for p in altered['pages'] if p['surface_id']=='settings:general')
            if mutate=='old_source':page['states'][0]['design_recipe'][1]['path']=c.ENTRY
            if mutate=='missing_state':page['states'].pop()
            if mutate=='fake_reference':page['states'][0]['reference']['status']='approved'
            if mutate=='alternate_candidate':page['source_supplement']['entry']='docs/design/unapproved/index.html'
            if mutate=='dependency':page['production']['dependency_sha256']['apps/desktop/src/features/preferences/PreferencesScreen.tsx']='0'*64
            self.assertTrue(c.validate_general_mapping(self.root,altered),mutate)

    def test_local_service_sheet_retains_ten_records_and_exact_responsibilities(self):
        page=next(p for p in self.document['pages'] if p['surface_id']=='overlay:ModelConfigScreen:5fb902a57e')
        self.assertEqual([*c.COMMON_STATES,'retired_surface_absent'],[row['state'] for row in page['states']])
        for row in page['states'][:-1]:
            if row['state'] in ('empty','loading','error'):
                self.assertFalse(row['applicable'])
                self.assertEqual([],row['required_layers'])
                self.assertEqual('not_applicable',row['design_status'])
                self.assertEqual({},row['execution'])
            else:
                self.assertTrue(row['applicable'])
                self.assertEqual(['real_react','packaged_native'],row['required_layers'])
                self.assertEqual('needs_mapping',row['mapping_status'])
                self.assertEqual('blocked',row['product_status'])

    def test_three_retired_sheets_keep_every_original_state_and_add_ast_absence(self):
        lineage=c.model_config_lineage(self.root)
        pages={page['surface_id']:page for page in self.document['pages']}
        self.assertEqual(3,len(lineage['legacy_surface_obligations']))
        self.assertEqual(36,sum(row['original_applicable_count'] for row in lineage['legacy_surface_obligations']))
        for original in lineage['legacy_surface_obligations']:
            page=pages[original['surface_id']]
            self.assertEqual('retired_surface_with_retained_obligations',page['surface_status'])
            self.assertFalse(page['production']['currently_discovered'])
            self.assertEqual(original['original_state_ids'],[row['id'] for row in page['states'][:-1]])
            self.assertEqual('retired_surface_absent',page['states'][-1]['state'])
            self.assertTrue(page['retirement']['inventory_absence_required'])
            for row in page['states']:
                if row['applicable']:
                    self.assertEqual(c.LAYERS,row['required_layers']);self.assertEqual('blocked',row['product_status'])
                    self.assertEqual(dict.fromkeys(c.LAYERS,'not_run'),row['execution'])
            for row in page['states'][:-1]:
                with self.subTest(state=row['id']):
                    altered=copy.deepcopy(self.document)
                    victim=next(p for p in altered['pages'] if p['surface_id']==original['surface_id'])
                    victim['states']=[s for s in victim['states'] if s['id']!=row['id']]
                    self.assertTrue(c.validate_model_config_lineage(self.root,altered))
        affected=c.affected_state_ids(self.root,['apps/desktop/src/features/model-config/ServiceSelector.tsx'])
        expected={row['id'] for original in lineage['legacy_surface_obligations'] for row in pages[original['surface_id']]['states'] if row['applicable']}
        self.assertTrue(expected<=affected)

    def test_retired_sheet_reintroduced_in_actual_exported_root_is_rejected(self):
        p=self.root/'apps/desktop/src/features/model-config/ModelConfigScreen.tsx';original=p.read_bytes()
        render='return <div className={styles.host} data-service-workspace data-guided-service-page>'
        for identity,title,variable in [('92d4a07590','应用这一版配置？','applyReview'),('d6953cce06','复制已保存的模型配置？','copyReview'),('5fb902a57e','添加服务','addReview')]:
            with self.subTest(surface=identity):
                fixture='<Sheet title="'+title+'" open={'+variable+' !== null} onClose={() => {}}>旧入口</Sheet>'
                p.write_text(original.decode().replace(render,render+fixture,1))
                self.assertIn('overlay:ModelConfigScreen:'+identity,{row['id'] for row in c.inventory(self.root)['pages']})
                self.assertRejected()
                p.write_bytes(original)
        self.assertEqual(original,p.read_bytes())

    def test_new_surfaces_use_exact_known_design_and_keep_different_dialogs_unmapped(self):
        # This retained regression owns the eight service-configuration entries,
        # not every unrelated surface added by later product iterations.
        previous={row['surface_id'] for row in c.model_config_lineage(self.root)['original_all_page_states']}
        new={page for page,spec in c.SPECS.items() if page not in previous
             and any('/features/model-config/' in path for path in spec['paths'])}
        self.assertEqual(8,len(new));self.assertTrue(new<={p['id'] for p in self.ast['pages']})
        pages={page['surface_id']:page for page in self.document['pages']}
        for key in ('overlay:ModelConfigScreen:3e3e52b7fc','overlay:ModelConfigScreen:6da213484c','overlay:SettingsWindow:ca82dd59c6'):
            self.assertTrue(all(row['mapping_status']=='needs_mapping' and row['product_status']=='blocked' for row in pages[key]['states'] if row['applicable']))
        for purpose in ('translation','grammar','context_word'):
            states={row['state']:row for row in pages['subpage:ModelConfigScreen:catalogPurpose:'+purpose]['states']}
            self.assertEqual('mapped_recipe',states['normal']['mapping_status'])
            self.assertIn(c.select('#guided-catalog-purpose',purpose),states['normal']['design_recipe'])
            self.assertFalse(states['error']['applicable']);self.assertEqual([],states['error']['required_layers'])
        selector={row['state']:row for row in pages['overlay:ServiceSelector:ddfdd2de73']['states']}
        self.assertEqual('mapped_recipe',selector['normal']['mapping_status'])
        for name in ('grammar_purpose','context_word_purpose','selection_failure'):
            self.assertEqual('needs_mapping',selector[name]['mapping_status'])

    def test_frozen_lineage_bytes_cannot_be_rewritten_to_waive_original_states(self):
        path=self.root/c.MODEL_CONFIG_LINEAGE;path.write_bytes(path.read_bytes()+b'\n')
        with self.assertRaisesRegex(ValueError,'frozen model-config obligation lineage changed'):
            c.expected_contract(self.root)
        self.assertRejected()

    def test_service_sheet_responsibility_records_cannot_hide_or_expand_obligations(self):
        key='overlay:ModelConfigScreen:5fb902a57e'
        for name in c.COMMON_STATES:
            with self.subTest(state=name):
                self.mutate(lambda d,n=name:next(p for p in d['pages'] if p['surface_id']==key)['states'].remove(next(s for s in next(p for p in d['pages'] if p['surface_id']==key)['states'] if s['state']==n)))
                self.assertRejected()
        for field,value in [('applicable',False),('required_layers',[]),('mapping_status','not_applicable'),('applicability_reason','metadata waiver')]:
            with self.subTest(field=field):
                self.mutate(lambda d,k=field,v=value:next(s for p in d['pages'] if p['surface_id']==key for s in p['states'] if s['state']=='disabled').update({k:v}))
                self.assertRejected()
        for field,value in [('applicable',True),('required_layers',['real_react']),('mapping_status','mapped'),('applicability_reason','source changed')]:
            with self.subTest(empty_field=field):
                self.mutate(lambda d,k=field,v=value:next(s for p in d['pages'] if p['surface_id']==key for s in p['states'] if s['state']=='loading').update({k:v}))
                self.assertRejected()

    def test_original_source_descriptors_are_stable_after_entity_tabs_are_removed(self):
        import current_reference_catalogue as references
        contract=c.expected_contract(self.root)
        new_sheet='overlay:ModelConfigScreen:5fb902a57e'
        originals={s['id'] for s in c.model_config_lineage(self.root)['original_all_page_states']}
        current_rows=[references.descriptor(s) for p in contract['pages'] if p['surface_id']!=new_sheet for s in p['states'] if s['applicable'] and s['id'] in originals]
        lineage_path=self.root/'apps/desktop/tests/fixtures/general-visual/legacy-general-descriptors.json'
        self.assertEqual('45d16a2ef9970b4bdd2f186d942df141f005786f4f8daec31eb50d71cd25ba27',hashlib.sha256(lineage_path.read_bytes()).hexdigest())
        historical={v['id']:v for v in json.loads(lineage_path.read_text())['descriptors']}
        self.assertEqual(14,len(historical))
        # Keep the original773/hash audit, reconstructing only the explicitly
        # superseded General14. This fixture never grants the new route a pass.
        rows=[historical.get(v['id'],v) for v in current_rows]
        non_general=[v for v in current_rows if v['id'] not in historical]
        self.assertEqual(759,len(non_general))
        self.assertEqual('de0e19cd923603423602dcfb3654b011e6e4e4fa95e714a384c59668dcdcdcb2',references.identity(non_general))
        self.assertEqual(773,len(rows))
        self.assertEqual('3734bef858a7aabde292109264535425639534a877d676d9eaf9101c2bd25d93',references.identity(rows))
        pages={p['surface_id']:p for p in contract['pages']}
        for key,p in pages.items():
            if key.startswith('subpage:ModelConfigScreen:tab:'):
                self.assertFalse(p['production']['currently_discovered'])
                self.assertEqual('settings',p['production']['role'])
                self.assertEqual('container',p['states'][0]['target']['container'])
                self.assertEqual([{'action':'assert_absent','selector':'[aria-label="模型配置实体"]'}],p['states'][0]['production_assertions'])
        # Recreate only the former production inventory facts. Authored source
        # descriptors must be identical before/after the production retirement.
        legacy=copy.deepcopy(self.ast)
        for key,p in pages.items():
            if key.startswith('subpage:ModelConfigScreen:tab:'):
                legacy['pages'].append({**p['production'],'role':'container'})
        with patch.object(c,'inventory',return_value=legacy):
            previous=c.expected_contract(self.root)
        previous_rows=[references.descriptor(s) for p in previous['pages'] if p['surface_id']!=new_sheet for s in p['states'] if s['applicable'] and s['id'] in originals]
        self.assertEqual(current_rows,previous_rows)
        previous_rows=[historical.get(v['id'],v) for v in previous_rows]
        self.assertEqual(rows,previous_rows)

    def test_capture_ast_lineage_preserves_every_original_source_descriptor_and_layers(self):
        from current_reference_catalogue import descriptor
        lineage=c.model_config_lineage(self.root)
        pages={page['surface_id']:page for page in self.document['pages']}
        old=c.CAPTURE_SURFACE_LINEAGE['legacy_surface'];new=c.CAPTURE_SURFACE_LINEAGE['current_surface']
        prior=[r for r in lineage['original_all_page_states'] if r['surface_id']==old]
        retained=pages[old]['states'][:-1]
        self.assertEqual(22,len(prior));self.assertEqual([r['id'] for r in prior],[r['id'] for r in retained])
        for before,after in zip(prior,retained):
            self.assertEqual(before['descriptor_sha256'],c.identity(descriptor(after)))
            self.assertEqual(before['required_layers'],after['required_layers'])
        self.assertEqual('retired_surface_absent',pages[old]['states'][-1]['state'])
        self.assertFalse(pages[old]['production']['currently_discovered'])
        self.assertEqual(new,pages[old]['retirement']['parent_surface'])
        for current in (new,c.CAPTURE_SURFACE_LINEAGE['connecting_surface']):
            self.assertTrue(pages[current]['production']['currently_discovered'])
            self.assertEqual(current,pages[current]['production']['id'])
            for row in pages[current]['states']:
                self.assertEqual('needs_mapping',row['mapping_status'])
                self.assertEqual('blocked',row['product_status'])
                self.assertEqual({'real_react':'not_run','packaged_native':'not_run'},row['execution'])

    def test_capture_connecting_missing_recipe_reference_or_layer_cannot_be_forged(self):
        for field,value in [('mapping_status','mapped_recipe'),('required_layers',['real_react']),('reference',{'status':'approved'})]:
            with self.subTest(field=field):
                self.mutate(lambda d:next(p for p in d['pages'] if p['surface_id']==c.CAPTURE_SURFACE_LINEAGE['connecting_surface'])['states'][0].update({field:value}))
                self.assertRejected()

    def test_capture_lineage_cannot_drop_old_state_or_retarget_to_unrelated_host(self):
        old=c.CAPTURE_SURFACE_LINEAGE['legacy_surface']
        self.mutate(lambda d:next(p for p in d['pages'] if p['surface_id']==old)['states'].pop(0));self.assertRejected()
        self.mutate(lambda d:next(p for p in d['pages'] if p['surface_id']==old)['retirement'].update(parent_surface='main:history'));self.assertRejected()

    def test_production_role_cannot_be_faked_to_match_a_source_capture_container(self):
        self.mutate(lambda d:next(p for p in d['pages'] if p['surface_id']=='subpage:ModelConfigScreen:tab:connections')['production'].update(role='container'))
        self.assertRejected()

    def test_actual_ast_and_honest_blocked_baseline(self):
        self.assertEqual(c.validate_contract(self.root),[])
        self.assertEqual(set(self.ast['main_pages']),set(self.ast['main_navigation']))
        self.assertEqual(set(self.ast['settings_pages']),set(self.ast['settings_navigation']))
        self.assertIn('surface:word-card',{p['id'] for p in self.ast['pages']})
        discovered={p['id'] for p in self.ast['pages']}
        retired={page for page,spec in c.SPECS.items() if spec.get('retired')}
        self.assertEqual(discovered|retired,set(c.SPECS))
        self.assertEqual({p['surface_id'] for p in self.document['pages']},set(c.SPECS))
        parent=next(p for p in self.document['pages'] if p['surface_id']=='main:session')
        for dataset,section in [('body','reading'),('extraction','texts'),('hook_events','logs')]:
            surface='subpage:ReadingPanel:dataset:'+dataset
            self.assertNotIn(surface,discovered)
            page=next(p for p in self.document['pages'] if p['surface_id']==surface)
            self.assertEqual(page['production']['kind'],'retired_surface_obligation')
            self.assertIs(page['production']['currently_discovered'],False)
            self.assertEqual(page['production']['dependency_paths'],parent['production']['dependency_paths'])
            self.assertEqual(page['retirement']['parent_surface'],'main:session')
            self.assertEqual(page['retirement']['replacement_section'],'details[data-flat-session-section="'+section+'"]')
            self.assertEqual([s['id'] for s in page['states']],[c._state_id(surface,'retired_surface_absent')])
        self.assertEqual(c.summary(self.root)['status'],'blocked')
        self.assertTrue(all(s['execution']==(dict.fromkeys(c.LAYERS,'not_run') if s['applicable'] else {}) for p in self.document['pages'] for s in p['states']))

    def test_preview_details_is_independent_and_keeps_unmapped_async_obligations(self):
        identity='overlay:PreviewDetails:c72c668099'
        self.assertIn(identity,{p['id'] for p in self.ast['pages']})
        page=next(p for p in self.document['pages'] if p['surface_id']==identity)
        states={s['state']:s for s in page['states']}
        self.assertEqual(page['production']['tag'],'Sheet')
        self.assertEqual(set(states),set(c.COMMON_STATES))
        for name in ('normal','light','narrow','zoom_200','keyboard_focus_escape'):
            self.assertEqual(states[name]['mapping_status'],'mapped_recipe')
            self.assertEqual(states[name]['execution'],dict.fromkeys(c.LAYERS,'not_run'))
            self.assertEqual(states[name]['reference']['status'],'missing_independent_reference')
            self.assertEqual(states[name]['product_status'],'blocked')
        for name in ('loading','error','long_text'):
            self.assertTrue(states[name]['applicable'])
            self.assertEqual(states[name]['mapping_status'],'needs_mapping')
        keyboard=states['keyboard_focus_escape']['design_recipe']
        self.assertIn({'action':'assert_focused','selector':'#window-preview [data-rw-action="preview-details"]'},keyboard)
        self.mutate(lambda d:d['pages'].remove(next(p for p in d['pages'] if p['surface_id']==identity)))
        self.assertRejected()

    def test_page_omission(self):
        self.mutate(lambda d:d['pages'].pop())
        self.assertRejected()

    def test_every_applicable_state_omission_is_rejected(self):
        # Exercise every obligation, without spawning a parser per deleted row.
        data=copy.deepcopy(self.document)
        self.assertEqual(c.obligation_errors(data),[])
        for page in data['pages']:
            for index,row in enumerate(list(page['states'])):
                if not row['applicable']: continue
                with self.subTest(state=row['id']):
                    removed=page['states'].pop(index)
                    self.assertTrue(c.obligation_errors(data))
                    page['states'].insert(index,removed)
        self.mutate(lambda d:d['pages'][0]['states'].pop(0))
        self.assertRejected()

    def test_new_main_page_requires_mapping(self):
        p=self.root/'apps/desktop/src/app/navigation.ts'; text=p.read_text()
        text=text.replace("export type MainPage = ","export type MainPage = 'new_review_page' | ",1)
        text=text.replace("export const mainNavigation:","export const mainNavigation:",1)
        text=text.replace("  { id: 'library',", "  { id: 'new_review_page', label: '新页', icon: 'grid', group: '工具' },\n  { id: 'library',",1)
        p.write_text(text)
        self.assertRejected()

    def test_union_and_settings_navigation_must_both_match(self):
        p=self.root/'apps/desktop/src/app/navigation.ts'; p.write_text(p.read_text().replace("export type SettingsPage = ","export type SettingsPage = 'missing_nav' | ",1))
        self.assertRejected()

    def test_new_independent_wordcard_surface_requires_mapping(self):
        p=self.root/'apps/desktop/src/app/bootstrap.tsx'
        p.write_text(p.read_text()+"\nconst secondaryCard = new URL(location.href).searchParams.get('surface') === 'secondary-card';\n")
        self.assertRejected()

    def test_new_overlay_requires_mapping(self):
        p=self.root/'apps/desktop/src/features/manual-input/ManualInputScreen.tsx'
        p.write_text(p.read_text()+"\nconst newOverlay = <Sheet title=\"新增弹层\" open={true} onClose={() => {}}>内容</Sheet>;\n")
        self.assertRejected()

    def test_new_shared_component_file_requires_mapping(self):
        p=self.root/'apps/desktop/src/ui/primitives/NewReviewCard.tsx'
        p.write_text('export const NewReviewCard = () => <section>new</section>;\n')
        self.assertRejected()

    def test_new_actual_model_subpage_requires_mapping(self):
        p=self.root/'apps/desktop/src/features/model-config/ModelConfigScreen.tsx'
        original=p.read_bytes();text=original.decode()
        anchor='  const [catalog, setCatalog] = useState<ServiceCatalog | null>'
        exported=text.index('export function ModelConfigScreen(')
        self.assertGreater(text.index(anchor),exported);self.assertEqual(text.count(anchor),1)
        text=text.replace(anchor,"  const [tab, setTab] = useState<'purposes' | 'review_new'>('purposes');\n"+anchor,1)
        render='return <div className={styles.host} data-service-workspace data-guided-service-page>'
        self.assertEqual(text.count(render),1);self.assertGreater(text.index(render),exported)
        fixture='<div role="group" aria-label="Review subview fixture"><button onClick={() => setTab(\'purposes\')}>Existing</button><button onClick={() => setTab(\'review_new\')}>New</button>{tab === \'purposes\' ? <span>Existing</span> : <span>New</span>}</div>'
        text=text.replace(render,render+fixture,1)
        p.write_text(text)
        discovered={page['id'] for page in c.inventory(self.root)['pages']}
        self.assertIn('subpage:ModelConfigScreen:tab:review_new',discovered)
        self.assertRejected()
        p.write_bytes(original)
        self.assertNotIn('subpage:ModelConfigScreen:tab:review_new',{page['id'] for page in c.inventory(self.root)['pages']})
        self.assertEqual(original,p.read_bytes())

    def assertSnapshotCannotHideMutation(self):
        with self.assertRaises(ValueError): c.write_contract(self.root)
        self.assertRejected()

    def test_named_export_component_with_actual_library_consumer(self):
        controls=self.root/'apps/desktop/src/ui/primitives/Controls.tsx'
        controls.write_text(controls.read_text()+'\nfunction ReviewAuditCard(){return <section>新增共享组件</section>;}\nexport {ReviewAuditCard};\n')
        library=self.root/'apps/desktop/src/features/library/LibraryScreen.tsx'
        text="import { ReviewAuditCard } from '../../ui/primitives/Controls';\n"+library.read_text()
        self.assertIn('return <>',text)
        library.write_text(text.replace('return <>','return <><ReviewAuditCard/>',1))
        self.assertSnapshotCannotHideMutation()

    def test_surface_parameter_alias_with_actual_render_branch(self):
        p=self.root/'apps/desktop/src/app/bootstrap.tsx'
        text=p.read_text().replace("  const transient =", "  const requestedSurface = new URL(location.href).searchParams.get('surface');\n  const secondary = requestedSurface === 'secondary-card';\n  const transient =",1)
        self.assertIn('{transient ?',text)
        p.write_text(text.replace('{transient ?','{secondary ? <WordCardSurface port={port}/> : transient ?',1))
        self.assertSnapshotCannotHideMutation()

    def test_inferred_use_state_navigation_with_actual_render(self):
        p=self.root/'apps/desktop/src/features/manual-input/ManualInputScreen.tsx'
        text=p.read_text().replace("  const [text, setText]", "  const [reviewPage, setReviewPage] = useState('one');\n  const [text, setText]",1)
        navigation="<nav><button onClick={() => setReviewPage('one')}>One</button><button onClick={() => setReviewPage('two')}>Two</button></nav>{reviewPage === 'one' ? <section>One body</section> : <section>Two body</section>}"
        self.assertIn('return <>',text)
        p.write_text(text.replace('return <>','return <>'+navigation,1))
        self.assertSnapshotCannotHideMutation()

    def test_named_overlay_alias_with_actual_render(self):
        p=self.root/'apps/desktop/src/features/manual-input/ManualInputScreen.tsx'
        text="import { Sheet as ReviewSheet } from '../../ui/primitives/Controls';\n"+p.read_text()
        p.write_text(text.replace('return <>','return <><ReviewSheet title="新增命名别名弹层" open={true} onClose={() => {}}>内容</ReviewSheet>',1))
        self.assertSnapshotCannotHideMutation()

    def test_namespace_overlay_alias_with_actual_render(self):
        p=self.root/'apps/desktop/src/features/manual-input/ManualInputScreen.tsx'
        text="import * as ReviewControls from '../../ui/primitives/Controls';\n"+p.read_text()
        p.write_text(text.replace('return <>','return <><ReviewControls.Sheet title="新增命名空间弹层" open={true} onClose={() => {}}>内容</ReviewControls.Sheet>',1))
        self.assertSnapshotCannotHideMutation()

    def test_lazy_state_initializer_and_named_navigation_handler(self):
        p=self.root/'apps/desktop/src/features/manual-input/ManualInputScreen.tsx'
        text=p.read_text().replace("  const [text, setText]", "  const [reviewPage, setReviewPage] = useState(() => 'one');\n  const showReviewTwo = () => setReviewPage('two');\n  const [text, setText]",1)
        navigation="<nav><button onClick={() => setReviewPage('one')}>One</button><button onClick={showReviewTwo}>Two</button></nav>{reviewPage === 'one' ? <section>One body</section> : <section>Two body</section>}"
        p.write_text(text.replace('return <>','return <>'+navigation,1))
        self.assertSnapshotCannotHideMutation()

    def test_numeric_inferred_navigation_with_actual_render(self):
        p=self.root/'apps/desktop/src/features/manual-input/ManualInputScreen.tsx'
        text=p.read_text().replace("  const [text, setText]", "  const [reviewIndex, setReviewIndex] = useState(0);\n  const [text, setText]",1)
        navigation='<nav><button onClick={() => setReviewIndex(0)}>One</button><button onClick={() => setReviewIndex(1)}>Two</button></nav>{reviewIndex === 0 ? <section>One body</section> : <section>Two body</section>}'
        p.write_text(text.replace('return <>','return <>'+navigation,1))
        self.assertSnapshotCannotHideMutation()

    def test_window_role_alias_with_actual_render_branch(self):
        p=self.root/'apps/desktop/src/app/WindowRoot.tsx'
        text=p.read_text().replace('role: WindowRole;',"role: WindowRole | 'review-new';",1)
        text=text.replace('  const { snapshot }', '  const activeRole = role;\n  const { snapshot }',1)
        self.assertIn("{role === 'hud'",text)
        p.write_text(text.replace("{role === 'hud'", "{activeRole === 'review-new' ? <HudWindow store={store}/> : role === 'hud'",1))
        self.assertSnapshotCannotHideMutation()

    def test_privacy_state_without_payload_audit_is_not_mapped(self):
        for surface,state in [('settings:diagnostics','redacted_diagnostic'),('main:diagnostics','redacted_details'),('settings:data','safe_export')]:
            row=next(s for p in self.document['pages'] if p['surface_id']==surface for s in p['states'] if s['state']==state)
            self.assertEqual(row['design_status'],'unmapped_design_state')
            self.assertIn(row['id'],c.unresolved_design_state_ids(self.root))

    def test_import_conflict_recipe_must_select_and_assert_distinct_choice(self):
        page=next(p for p in self.document['pages'] if p['surface_id']=='settings:data')
        rows={s['state']:s for s in page['states']}
        self.assertNotEqual(rows['import_conflict']['design_recipe'],rows['import_preflight']['design_recipe'])
        assertions=[s for s in rows['import_conflict']['design_recipe'] if s['action'].startswith('assert_')]
        self.assertTrue(any(s['action']=='assert_text_contains' and s['value']=='替换同名草稿' for s in assertions))
        self.assertTrue(any(s['action']=='assert_design_predicate' and "importConflict === 'replace'" in s['expression'] for s in assertions))

    def test_shared_dependency_omission(self):
        path='apps/desktop/src/ui/primitives/Select.tsx'
        page=next(p for p in self.document['pages'] if path in p['production']['dependency_paths'])
        self.mutate(lambda d:next(p for p in d['pages'] if p['surface_id']==page['surface_id'])['production']['dependency_paths'].remove(path))
        self.assertRejected()

    def test_shared_select_and_tokens_cover_actual_consumers(self):
        selected=c.affected_state_ids(self.root,['apps/desktop/src/ui/primitives/Select.tsx'])
        for surface in ('main:library','settings:translation','window:hud'):
            self.assertIn(c._state_id(surface,'normal'),selected)
        self.assertEqual(c.affected_state_ids(self.root,['apps/desktop/src/ui/tokens/tokens.json']),c.STATE_IDS)
        self.assertEqual(c.affected_state_ids(self.root,['apps/desktop/src-tauri/src/platform/unknown-new-native.rs']),c.STATE_IDS)
        self.assertEqual(c.affected_state_ids(self.root,['docs/README.md']),frozenset())

    def test_unreferenced_tests_and_docs_do_not_expand_production_obligations(self):
        paths=['apps/desktop/tests/visual/capture-reviewed-states.mjs',
               'apps/desktop/src/ui/Select.test.tsx','apps/desktop/src/features/reading/__tests__/fixture.ts',
               'README.md','docs/native/notes.md','scripts/quality/docs/notes.md','scripts/quality/verify.py']
        self.assertEqual(frozenset(),c.affected_state_ids(self.root,paths))
        # These were formerly included in the empty-impact assertion. Their
        # product roots have no reviewed documentation exclusion from packaging.
        for path in ['apps/desktop/README.md','crates/app-core/docs/notes.md']:
            with self.subTest(path=path):
                self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[path]))

    def test_production_imported_test_fixture_keeps_actual_consumer_obligations(self):
        fixture='apps/desktop/tests/production-fixture.test.ts'
        path=self.root/fixture; path.parent.mkdir(parents=True,exist_ok=True)
        path.write_text('export const productionFixture = 1;\n')
        reading=self.root/'apps/desktop/src/features/reading/ReadingPanel.tsx'
        reading.write_text(reading.read_text()+"\nimport '../../../tests/production-fixture.test';\n")
        selected=c.affected_state_ids(self.root,[fixture])
        self.assertIn(c._state_id('main:session','normal'),selected)
        self.assertIn(c._state_id('subpage:ReadingPanel:dataset:body','retired_surface_absent'),selected)
        self.assertNotEqual(c.STATE_IDS,selected)

    def test_optional_entry_dependency_mode_preserves_frozen_inventory_shape(self):
        actual=c.inventory(self.root,production_dependencies=True)
        self.assertEqual(['apps/desktop/src/app/bootstrap.tsx'],actual.pop('production_entry_roots'))
        self.assertIn('apps/desktop/src/app/bootstrap.tsx',actual.pop('production_dependency_paths'))
        self.assertEqual(self.ast,actual)

    def test_bootstrap_imported_test_and_docs_cannot_be_dropped_by_path_names(self):
        bootstrap=self.root/'apps/desktop/src/app/bootstrap.tsx'
        original=bootstrap.read_text()
        for fixture,statement,content in [
            ('apps/desktop/tests/bootstrap-fixture.test.ts',"import '../../tests/bootstrap-fixture.test';",'export const fixture = 1;\n'),
            ('apps/desktop/docs/runtime-data.json',"import '../../docs/runtime-data.json';",'{"runtime":true}\n'),
            ('docs/runtime-data.json',"import '../../../../docs/runtime-data.json';",'{"runtime":true}\n'),
            ('scripts/quality/docs/runtime-fixture.test.ts',"import '../../../../scripts/quality/docs/runtime-fixture.test';",'export const fixture = 1;\n'),
            ('apps/desktop/tests/lazy-fixture.test.ts',"void import('../../tests/lazy-fixture.test');",'export const fixture = 1;\n')]:
            with self.subTest(fixture=fixture):
                path=self.root/fixture; path.parent.mkdir(parents=True,exist_ok=True);path.write_text(content)
                bootstrap.write_text(original+'\n'+statement+'\n')
                self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[fixture]))
        bootstrap.write_text(original)

    def test_doc_directory_test_name_still_requires_actual_entry_graph_exclusion(self):
        fixture='apps/desktop/src/docs/runtime.test.ts'
        path=self.root/fixture; path.parent.mkdir(parents=True,exist_ok=True)
        path.write_text('export const productionFixture = 1;\n')
        self.assertEqual(frozenset(),c.affected_state_ids(self.root,[fixture]))
        bootstrap=self.root/'apps/desktop/src/app/bootstrap.tsx'
        bootstrap.write_text(bootstrap.read_text()+"\nimport '../docs/runtime.test';\n")
        self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[fixture]))

    def test_nested_doc_names_never_exempt_unknown_product_or_native_inputs(self):
        paths=['native/docs/window.swift','crates/app-infra/src/docs/helpers.rs',
               'apps/desktop/src-tauri/src/doc/window.cpp','native/docs/tests/window.test.hpp',
               'crates/app-infra/src/doc/test_helpers.rs','unknown-root/doc/runtime.inc',
               'unknown-root/docs/runtime.inl','unknown-root/doc/test_runtime.ipp',
               'packages/foo/docs/asset.js','resources/docs/asset.bin','assets/doc/icon.svg',
               'runtime/doc/x','apps/desktop/src/docs/new.ts']
        for path in paths:
            with self.subTest(path=path):
                self.assertFalse(c._nonproduction_path(path))
                self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[path]))

    def test_exact_quality_recompute_identity_has_real_independent_dispatch_caller(self):
        comparator='scripts/contracts/compare_product_capture.mjs'
        self.assertEqual(frozenset({comparator}),c.QUALITY_ONLY_PATHS)
        self.assertEqual(frozenset(),c.affected_state_ids(self.root,[comparator]))
        for adjacent in ['scripts/contracts/unknown.mjs','scripts/contracts/compare_product_capture-v2.mjs']:
            with self.subTest(path=adjacent): self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[adjacent]))
        # Bind the exception to the real verify-ui independent recomputation
        # dispatch, not a report label or a metadata-supplied command.
        tree=ast.parse((c.ROOT/'scripts/quality/product_execution.py').read_text())
        dispatch=next(node for node in tree.body if isinstance(node,ast.FunctionDef) and node.name=='run_product_check')
        assignment=next(node for node in ast.walk(dispatch) if isinstance(node,ast.Assign)
                        and any(isinstance(target,ast.Name) and target.id=='comparison_command' for target in node.targets))
        self.assertIsInstance(assignment.value,ast.List)
        self.assertEqual(['node',comparator],[ast.literal_eval(value) for value in assignment.value.elts[:2]])
        self.assertTrue(any(isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute)
                            and isinstance(node.func.value,ast.Name) and node.func.value.id=='subprocess'
                            and node.func.attr=='run' and node.args and isinstance(node.args[0],ast.Name)
                            and node.args[0].id=='comparison_command' for node in ast.walk(dispatch)))

    def test_quality_recompute_identity_cannot_override_a_real_production_import(self):
        comparator='scripts/contracts/compare_product_capture.mjs'
        path=self.root/comparator;path.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(c.ROOT/comparator,path)
        bootstrap=self.root/'apps/desktop/src/app/bootstrap.tsx'
        bootstrap.write_text(bootstrap.read_text()+"\nimport '../../../../scripts/contracts/compare_product_capture.mjs';\n")
        self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[comparator]))

    def test_unresolved_production_dynamic_import_fails_closed(self):
        bootstrap=self.root/'apps/desktop/src/app/bootstrap.tsx'
        bootstrap.write_text(bootstrap.read_text()+"\nconst unknownModule = location.hash; void import(unknownModule);\n")
        with self.assertRaisesRegex(ValueError,'non-enumerable dynamic module import'):
            c.affected_state_ids(self.root,['apps/desktop/tests/unknown.test.ts'])

    def test_unknown_production_roots_and_native_remain_conservative(self):
        for path in ['new-product-package/src/new.ts','native/new-runtime.mm','native/tests/core_fixture.cpp',
                     'new-native/tests/runtime.rs','new-product-package/asset.bin','scripts/build_desktop.py',
                     'crates/app-core/tests/native_fixture.rs','apps/desktop/src-tauri/tests/window.test.rs',
                     'apps/desktop/src/unknown-new.tsx','resources/new-resource.bin']:
            with self.subTest(path=path):
                self.assertEqual(c.STATE_IDS,c.affected_state_ids(self.root,[path]))

    def test_typed_assessment_binds_actual_gaps_and_exit_without_product_execution(self):
        report=c.summary(self.root)
        self.assertEqual([],c.assessment_errors(self.root,report,1))
        self.assertEqual('blocked',report['status'])
        self.assertTrue(report['release_gaps'])
        self.assertEqual(len(c.STATE_IDS),report['applicable_state_count'])
        self.assertIs(report['product_tests_executed'],False)
        self.assertIs(report['product_alignment_approved'],False)
        mutations=[('schema_version',True),('status',{}),('status',[]),('applicable_state_count',True),
                   ('applicable_state_count',report['applicable_state_count']-1),
                   ('release_gaps',[]),('release_gaps',['fabricated gap']),
                   ('metadata_errors',{}),('target_id','acceptance-v4'),
                   ('composition_id','legacy-guided-only'),('independent_references','approved'),
                   ('production_execution','passed'),('product_tests_executed',True),
                   ('product_alignment_approved',True)]
        with patch.object(c,'summary',return_value=report):
            for key,value in mutations:
                with self.subTest(key=key,value=value):
                    changed=copy.deepcopy(report); changed[key]=value
                    self.assertTrue(c.assessment_errors(self.root,changed,1))
            self.assertTrue(c.assessment_errors(self.root,report,0))
            changed=copy.deepcopy(report); changed.update(status='passed',release_gaps=[])
            self.assertTrue(c.assessment_errors(self.root,changed,0))

    def test_retired_reading_states_keep_actual_parent_dependency_impact(self):
        parent=next(p for p in self.ast['pages'] if p['id']=='main:session')
        retired={c._state_id('subpage:ReadingPanel:dataset:'+dataset,'retired_surface_absent') for dataset in ('body','extraction','hook_events')}
        self.assertTrue(retired<=c.STATE_IDS)
        for path in ('apps/desktop/src/features/reading/ReadingPanel.tsx','apps/desktop/src/features/reading/SessionRecordPanel.tsx','apps/desktop/src/features/reading/styles.module.css'):
            with self.subTest(path=path):
                self.assertIn(path,parent['dependency_paths'])
                affected=c.affected_state_ids(self.root,[path])
                self.assertTrue(retired<=affected)
                self.assertIn(c._state_id('main:session','body_expanded'),affected)

    def test_retired_reading_replacement_mapping_cannot_be_changed(self):
        for dataset in ('body','extraction','hook_events'):
            with self.subTest(dataset=dataset):
                surface='subpage:ReadingPanel:dataset:'+dataset
                self.mutate(lambda d:next(p for p in d['pages'] if p['surface_id']==surface)['retirement'].update(replacement_section='details[data-flat-session-section="wrong"]'))
                self.assertRejected()

    def test_nonretired_main_navigation_removal_is_rejected(self):
        p=self.root/'apps/desktop/src/app/navigation.ts'
        text,removed=re.subn(r"^\s*\{ id: 'session',[^\n]*\n",'',p.read_text(),count=1,flags=re.M)
        self.assertEqual(removed,1)
        p.write_text(text)
        self.assertRejected()

    def test_hidden_target_metadata(self):
        self.mutate(lambda d:d['pages'][0]['states'][0]['target'].update(visibility='hidden'))
        self.assertRejected()

    def test_historical_reference_cannot_be_promoted(self):
        self.mutate(lambda d:d['pages'][0]['states'][0]['reference'].update(status='approved',path='docs/design/first-product-v1/acceptance-v4/reading-hud.top.png',sha256='a'*64))
        self.assertRejected()

    def test_fake_product_approval_and_adapter(self):
        self.mutate(lambda d:d.update(product_status='passed',production_capture_adapter='prototype-html'))
        self.assertRejected()

    def test_skip_product_layer_or_fake_execution(self):
        self.mutate(lambda d:d['pages'][0]['states'][0].update(required_layers=['real_react'],execution={'real_react':'passed'}))
        self.assertRejected()

    def test_source_anchored_missing_state_cannot_be_promoted(self):
        # The complete current baseline has no audited missing claim. Preserve the
        # negative by injecting a separately source-anchored omission in isolation.
        # Use a new unmapped state: the frozen original809 states may not be
        # rewritten, even by an isolated guard fixture. This source audit only
        # exercises evidence binding and grants no new design approval.
        key=('overlay:ModelConfigScreen:6da213484c','revoke_failed')
        gap={'reason':'隔离审计：当前settings的guardedNavigate确认仅有保存/放弃导航，没有Google撤销失败Sheet。',
             'path':c.ENTRY.replace('index.html','settings.js'),
             'anchor':'function guardedNavigate(app,change)',
             'quote':"confirmLabel:'保存并切换',cancelLabel:'继续编辑'"}
        self.assertEqual(c.missing_design_state_ids(self.root),frozenset())
        with patch.dict(c.VERIFIED_DESIGN_GAPS,{key:gap}):
            expected=c.expected_contract(self.root)
            (self.root/c.CONTRACT).write_text(json.dumps(expected,ensure_ascii=False))
            self.assertEqual(c.validate_contract(self.root),[])
            state_id=c._state_id(*key)
            self.assertEqual(c.missing_design_state_ids(self.root,{state_id}),frozenset({state_id}))
            row=next(s for p in expected['pages'] if p['surface_id']==key[0] for s in p['states'] if s['state']==key[1])
            self.assertEqual(row['design_status'],'missing_design_state')
            self.assertTrue(row['gap_evidence']['source_sha256'])
            row.update(design_status='existing_design_state',design_gap='',gap_evidence=None)
            (self.root/c.CONTRACT).write_text(json.dumps(expected,ensure_ascii=False))
            self.assertRejected()
        with patch.dict(c.VERIFIED_DESIGN_GAPS,{key:{**gap,'anchor':'nonexistent detail failure source anchor'}}):
            expected=c.expected_contract(self.root)
            (self.root/c.CONTRACT).write_text(json.dumps(expected,ensure_ascii=False))
            self.assertTrue(any('actual source reason and anchor' in e for e in c.validate_contract(self.root)))

    def test_complete_history_mapping_uses_current_hx_entry_without_losing_query(self):
        page=next(p for p in self.document['pages'] if p['surface_id']=='main:history')
        rows={s['state']:s for s in page['states']}
        self.assertEqual(len(rows),20)
        self.assertEqual({n for n,r in rows.items() if r['design_status']=='unmapped_design_state'},
                         {'detail_loading','detail_error','correction_conflict'})
        self.assertEqual(sum(r['design_status']=='existing_design_state' for r in rows.values()),17)
        for row in rows.values():
            nav=[step for step in row['design_recipe'] if step['action']=='navigate']
            self.assertEqual(len(nav),1)
            self.assertEqual(nav[0]['path'],c.HISTORY_ENTRY)
            self.assertEqual(nav[0]['query']['scenario'],'history')
        self.assertEqual(rows['long_text']['design_recipe'][0]['query']['historyState'],'long')
        self.assertEqual(rows['light']['design_recipe'][0]['query'],{'scenario':'history','theme':'light','historyState':'default'})
        self.assertIn({'action':'set_viewport','width':740,'height':1200},rows['narrow']['design_recipe'])
        self.assertTrue(any(step.get('script','').startswith('document.documentElement.style.zoom="2"') for step in rows['zoom_200']['design_recipe']))

    def test_complete_composition_fixture_and_authority_omissions_are_rejected(self):
        self.assertEqual(len(c.DESIGN_FIXTURE_SHA256),62)
        self.assertIn('docs/design/first-product-v1/assets/wallpaper-frame.png',c.DESIGN_FIXTURE_SHA256)
        self.assertEqual(self.document['composition_id'],c.authority.CURRENT_COMPOSITION_ID)
        self.assertEqual(self.document['source_approval']['status'],'approved_by_user')
        self.mutate(lambda d:d['authority_source_sha256'].pop(c.authority.CURRENT_RECORD))
        self.assertRejected()
        self.mutate(lambda d:d['design_fixture_source_sha256'].pop(c.HISTORY_ENTRY.replace('index.html','history.css')))
        self.assertRejected()

    def test_no_recipe_is_not_evidence_of_missing_design(self):
        row=next(s for p in self.document['pages'] if p['surface_id']=='settings:translation' for s in p['states'] if s['state']=='required_fields')
        self.assertEqual(row['design_status'],'unmapped_design_state')
        self.assertEqual(row['mapping_status'],'needs_mapping')
        self.assertIsNone(row['gap_evidence'])
        self.assertEqual(row['design_gap'],'')
        self.mutate(lambda d:next(s for p in d['pages'] if p['surface_id']=='settings:translation' for s in p['states'] if s['state']=='required_fields').update(design_status='missing_design_state'))
        self.assertRejected()

    def test_retired_tabs_must_not_be_recreated(self):
        retired=[p for p in self.document['pages'] if p['surface_status']=='retired_production_surface']
        self.assertTrue(retired)
        for page in retired:
            self.assertEqual([s['state'] for s in page['states']],['retired_surface_absent'])
            self.assertEqual(page['retirement']['policy'],'merge_into_current_guided_sections_never_recreate_legacy_tab')
            self.assertTrue(page['states'][0]['production_assertions'])
            self.assertTrue(all(a['action']=='assert_absent' for a in page['states'][0]['production_assertions']))
        self.mutate(lambda d:next(p for p in d['pages'] if p['surface_status']=='retired_production_surface')['retirement'].update(policy='recreate_legacy_tabs'))
        self.assertRejected()

    def test_retired_navigation_may_disappear_without_recreating_old_tabs(self):
        p=self.root/'apps/desktop/src/features/model-config/ModelConfigScreen.tsx'
        original=p.read_text()
        self.assertNotIn('aria-label="模型配置实体"',original)
        anchor='  const [catalog, setCatalog] = useState<ServiceCatalog | null>'
        self.assertEqual(original.count(anchor),1)
        self.assertGreater(original.index(anchor),original.index('export function ModelConfigScreen('))
        model_state="  const [tab, setTab] = useState<'purposes' | 'connections' | 'models' | 'parameters' | 'runtime' | 'terms'>('purposes');\n"
        render='return <div className={styles.host} data-service-workspace data-guided-service-page>'
        self.assertEqual(original.count(render),1)
        model_nav='<div role="group" aria-label="模型配置实体"><button onClick={() => setTab(\'purposes\')}>Navigation</button><button onClick={() => setTab(\'connections\')}>Navigation</button><button onClick={() => setTab(\'models\')}>Navigation</button><button onClick={() => setTab(\'parameters\')}>Navigation</button><button onClick={() => setTab(\'runtime\')}>Navigation</button><button onClick={() => setTab(\'terms\')}>Navigation</button>{tab === \'purposes\' ? <span>Purpose</span> : <span>Other</span>}</div>'
        text=original.replace(anchor,model_state+anchor,1).replace(render,render+model_nav,1)
        p.write_text(text)
        model_surfaces={'subpage:ModelConfigScreen:tab:'+name for name in ('purposes','connections','models','parameters','runtime','terms')}
        self.assertTrue(model_surfaces<={page['id'] for page in c.inventory(self.root)['pages']})
        self.assertEqual(text.count(model_nav),1)
        text=text.replace(model_nav,'',1).replace(model_state,'',1)
        self.assertEqual(original,text)
        p.write_text(text)
        reading=self.root/'apps/desktop/src/features/reading/ReadingPanel.tsx'
        # The product has already removed these tabs. Recreate the historical
        # navigation only in this isolated fixture, prove AST discovery, then
        # remove it. A zero-replacement match would not exercise retirement.
        reading_original=reading.read_bytes();body=reading_original.decode()
        state_anchor='  const datasets: { id: SessionDataset; label: string }[]'
        self.assertEqual(body.count(state_anchor),1)
        dataset_state="  const [dataset, setDataset] = useState<SessionDataset>('body');\n"
        body=body.replace(state_anchor,dataset_state+state_anchor,1)
        navigation='<div className={styles.sessionTabs} role="tablist" aria-label="当前会话内容">{datasets.map(item => <button key={item.id} onClick={() => setDataset(item.id)}>{item.label}</button>)}</div>'
        render_anchor='      <div className={`${styles.readingGrid} ${stopped ? styles.stoppedGrid : \'\'}`} data-verified-window={bound}>'
        self.assertEqual(body.count(render_anchor),1)
        branch="{dataset === 'body' ? <span>正文历史</span> : dataset === 'extraction' ? <span>全部提取</span> : <span>Hook 日志</span>}\n"
        body=body.replace(render_anchor,navigation+branch+render_anchor,1)
        reading.write_text(body)
        reading_surfaces={'subpage:ReadingPanel:dataset:'+value for value in ('body','extraction','hook_events')}
        self.assertTrue(reading_surfaces<={page['id'] for page in c.inventory(self.root)['pages']})
        self.assertEqual(body.count(navigation+branch),1)
        body=body.replace(navigation+branch,'',1).replace(dataset_state,'',1)
        self.assertEqual(reading_original.decode(),body)
        reading.write_bytes(reading_original)
        self.assertEqual(reading_original,reading.read_bytes())
        discovered={page['id'] for page in c.inventory(self.root)['pages']}
        self.assertFalse(any(page.startswith('subpage:ModelConfigScreen:tab:') or page.startswith('subpage:ReadingPanel:dataset:') for page in discovered))
        # An explicitly reviewed metadata snapshot may retain the negative removal
        # obligations after the navigation is absent. Product execution stays blocked.
        c.write_contract(self.root)
        self.assertEqual(c.validate_contract(self.root),[])
        document=c._read(self.root/c.CONTRACT)
        retired=[page for page in document['pages'] if page['surface_status']=='retired_production_surface']
        self.assertTrue(all(not page['production']['currently_discovered'] for page in retired))
        self.assertEqual(document['product_status'],'blocked')

    def test_terms_responsibility_survives_retired_tab(self):
        row=next(s for p in self.document['pages'] if p['surface_id']=='settings:translation' for s in p['states'] if s['state']=='term_editing_mapping')
        self.assertEqual(row['design_status'],'unmapped_design_state')
        self.assertIn('MC-09',row['source_case_ids'])
        self.assertTrue(any(path.endswith('/TermSetEditor.tsx') for path in row['component_paths']))
        self.assertIn(row['id'],c.unresolved_design_state_ids(self.root))

    def test_presentational_props_do_not_invent_async_states(self):
        for name in ('Icon','PageHeader','FormRow','StatusLabel'):
            page=next(p for p in self.document['pages'] if p['surface_id']=='shared:'+name)
            for state in ('loading','error','empty','keyboard_focus_escape'):
                row=next(s for s in page['states'] if s['state']==state)
                self.assertFalse(row['applicable'])
                self.assertNotIn(row['id'],c.STATE_IDS)
        for page in self.document['pages']:
            for row in page['states']:
                if row['state_kind']=='visual_variant': self.assertNotEqual(row['design_status'],'missing_design_state')

    def test_fixture_source_drift_is_not_refreshed(self):
        p=self.root/'docs/design/first-product-v1/app.js'; p.write_text(p.read_text()+'\n// unreviewed drift\n')
        self.assertRejected()

    def test_duplicate_json_and_non_json_constant_are_rejected(self):
        p=self.root/c.CONTRACT
        p.write_text('{"pages":[],"pages":[]}'); self.assertRejected()
        p.write_text('{"pages":NaN}'); self.assertRejected()

    def test_symlink_ancestor_and_unsafe_path_are_rejected(self):
        directory=self.root/'docs/design/first-product-v1'; moved=self.root/'design-backup'; directory.rename(moved); directory.symlink_to(moved,target_is_directory=True)
        self.assertRejected()
        for value in ('../escape','/absolute','apps/desktop/\nsecret','apps//desktop'):
            with self.subTest(path=value),self.assertRaises(ValueError): c._safe(self.root,value)


if __name__=='__main__': unittest.main()
