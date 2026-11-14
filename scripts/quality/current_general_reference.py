"""Separate, independently reviewed General whole-client reference catalogue.

The guided v1/v2 packs remain immutable lineage. They cannot establish current
General V3+approved pending/error states or install an arbitrary source/renderer.
"""
from __future__ import annotations
from datetime import datetime
import hashlib
import json
from pathlib import Path
from gate_core import safe_path,digest
from current_general_visual_authority import STATE_IDS,LAYERS,authority,capture_cases,case_key,reference_inputs,closure_sha,SCOPE
CATALOGUE='docs/development/current-general-reference-catalogue.json'
REVIEW='docs/development/current-general-reference-review.json'
AUTHOR_IDS=frozenset(('/root','/root/general_phase2_reference_adapter','/root/general_native_isolated_renderer','/root/general_pending_error_ui','/root/general_native_renderer_design','/root/general_state_design','/root/general_missing_state_design','/root/session_navigation_contract_review','/root/general_exact_visual_repair','/root/general_native_observation_debug'))

def load(path):
 def unique(pairs):
  value={}
  for k,v in pairs:
   if k in value:raise ValueError('Duplicate General evidence key')
   value[k]=v
  return value
 return json.loads(Path(path).read_text(),object_pairs_hook=unique,parse_constant=lambda v:(_ for _ in ()).throw(ValueError('Nonfinite JSON')))

def artifact(root,ref):
 if not isinstance(ref,dict) or set(ref)!={'path','sha256'}:raise ValueError('Exact General artifact reference required')
 p=safe_path(root,ref['path'])
 if not p.is_file() or digest(p)!=ref['sha256']:raise ValueError('General original artifact identity changed')
 return p

def assess(root,required=None,layer=None):
 root=Path(root);wanted=set(STATE_IDS if required is None else required)
 result={'schema_version':1,'kind':'general-reference-assessment/v1','status':'blocked','scope':SCOPE,'errors':[],'gaps':[],
         'approved_state_ids':[],'product_alignment_approved':False,'native_verified':False,'catalogue':None,'review':None}
 try:
  if not wanted<=set(STATE_IDS):raise ValueError('Unknown General reference state')
  if layer is not None and layer not in LAYERS:raise ValueError('Unknown General reference layer')
  cp=safe_path(root,CATALOGUE);rp=safe_path(root,REVIEW)
  if not cp.is_file() or not rp.is_file():result['gaps'].append('General independent whole-client reference catalogue/review missing');return result
  c,r=load(cp),load(rp);a=authority(root)
  if c.get('kind')!='general-independent-reference-catalogue/v1' or c.get('source_authority')!=a or c.get('scope')!=SCOPE:raise ValueError('General catalogue source/context/state authority differs')
  if r.get('kind')!='general-independent-reference-review/v1' or r.get('status')!='approved' or r.get('catalogue')!={'path':CATALOGUE,'sha256':digest(cp)}:raise ValueError('Exact General independent review absent')
  if any(r.get(k) is not False for k in ('product_alignment_approved','native_verified','game_tests_executed','os_runtime_verified','storage_runtime_verified')):raise ValueError('Reference review exceeds source-rendering capability')
  reviewer=r.get('reviewer_id');authors=set(r.get('author_ids',[]))
  if not AUTHOR_IDS<=authors or not isinstance(reviewer,str) or not reviewer.strip() or reviewer in authors:raise ValueError('Reference reviewer must be independent from every source/renderer/product author')
  reviewed=datetime.fromisoformat(r['reviewed_at'].replace('Z','+00:00'))
  if reviewed.tzinfo is None:raise ValueError('Timezone-aware post-generation review required')
  layers=(layer,) if layer else LAYERS
  expected={case_key(v):v for v in capture_cases()};reviewed_artifacts=[]
  for layer_name in layers:
   record=c.get('layers',{}).get(layer_name)
   if not record:result['gaps'].append('General reference layer absent: '+layer_name);continue
   generation_path=artifact(root,record['generation']);g=load(generation_path)
   if g.get('kind')!='general-reference-candidates/v1' or g.get('layer')!=layer_name or g.get('authority')!=a:raise ValueError('General generation source or renderer differs')
   generated=datetime.fromisoformat(g['finished_at'].replace('Z','+00:00'))
   if generated.tzinfo is None or reviewed<generated:raise ValueError('General review predates actual generation')
   # Bind the source producer epoch, not the final product epoch: a later actual
   # candidate cannot make its own reference a recursive build prerequisite.
   inputs=g.get('producer_inputs')
   if inputs!=reference_inputs(root,layer_name):raise ValueError('Reference source/finite producer assets changed since generation')
   if layer_name=='packaged_native':
    from current_general_native_display import approved_environment,validate_generation_environment
    native_environment,environment_approval=approved_environment(root)
    checked_environment,environment_lock=validate_generation_environment(root,g.get('captures',[]),'reference',native_environment)
    if record.get('environment')!=checked_environment or record.get('environment_lock')!=environment_lock:
     raise ValueError('Native layer environment differs from actual fixed27 source batch')
    binding=g.get('native_source_binding')
    if not isinstance(binding,dict) or binding!=native_source_binding(root,binding['source_map'],binding['sealed_source_tree_sha256']):raise ValueError('Native reference S1 embedded/current external producer epochs differ')
    for captured in g.get('captures',[]):
     original=load(safe_path(root,captured['raw_directory'])/'capture.json')
     if original.get('source_tree_sha256')!=binding['sealed_source_tree_sha256'] or original.get('package',{}).get('source_tree_sha256')!=binding['sealed_source_tree_sha256']:
      raise ValueError('Native source original came from a different sealed S1 producer package')
   rows=record.get('entries',[])
   if len(rows)!=27 or {v['case_key'] for v in rows}!=set(expected):raise ValueError('General fixed 27-case reference closure incomplete')
   generation_cases={case_key(v['case']):v for v in g.get('captures',[])}
   if set(generation_cases)!=set(expected):raise ValueError('Reference generation omitted a fixed case')
   for row in rows:
    key=row['case_key']
    if row.get('case')!=expected[key] or row.get('layer')!=layer_name or row.get('capture_scope')!='whole_client' or row.get('renderer')!=('public-WKWebView' if layer_name=='real_react' else 'signed-package-isolated-WKWebView'):raise ValueError('Reference dimensions/case/renderer differs')
    generated_row=generation_cases[key]
    raw_directory=generated_row.get('raw_directory')
    if not isinstance(raw_directory,str) or not raw_directory.startswith('.build/'):
     raise ValueError('General reference original producer directory absent')
    expected_png=raw_directory+('/document-1-frame-1.png' if layer_name=='real_react' else '/frame-1.png')
    expected_facts=raw_directory+('/native-result.json' if layer_name=='real_react' else '/observation.json')
    if row['png']['path']!=expected_png or row['facts']['path']!=expected_facts:
     raise ValueError('General reference substituted an ungenerated original frame/facts slot')
    frame=artifact(root,row['png']);facts=artifact(root,row['facts'])
    from current_general_visual_recompute import reference_observation
    reference_observation(root,row,layer_name)
    if row['png']['path'] not in r.get('reviewed_png_sha256',{}) or r['reviewed_png_sha256'][row['png']['path']]!=digest(frame):raise ValueError('Reference original PNG not explicitly reviewed')
    if row['facts']['path'] not in r.get('reviewed_facts_sha256',{}) or r['reviewed_facts_sha256'][row['facts']['path']]!=digest(facts):raise ValueError('Reference facts not explicitly reviewed')
    reviewed_artifacts.append(row['png']['path'])
   result['approved_state_ids']+=list(wanted)
  result['catalogue']={'path':CATALOGUE,'sha256':digest(cp)};result['review']={'path':REVIEW,'sha256':digest(rp),'reviewer_id':reviewer}
  result['status']='passed' if not result['gaps'] else 'blocked';result['approved_state_ids']=sorted(set(result['approved_state_ids']))
 except (OSError,ValueError,KeyError,TypeError,AttributeError) as error:result['errors'].append('General reference invalid: '+str(error));result['status']='failed'
 return result

def reference_gaps(root,required=None,layer=None):
 r=assess(root,required,layer);return r['errors']+r['gaps']

def entries(root,layer):
 r=assess(root,STATE_IDS,layer)
 if r['errors'] or r['gaps']:raise ValueError('; '.join(r['errors']+r['gaps']))
 c=load(Path(root)/CATALOGUE)
 return {v['case_key']:v for v in c['layers'][layer]['entries']}

# These inputs actually build the isolated Native reference renderer/assets.
# External capture/inspection scripts remain a separately recorded current
# producer epoch; they are not falsely described as bytes embedded in S1.
from current_general_visual_authority import PREFIX,RENDER_FILES,REFERENCE_REPLAY,SOURCE_BUILD_HELPER
NATIVE_EMBEDDED_INPUTS=tuple(PREFIX+name for name in RENDER_FILES)+(
 REFERENCE_REPLAY,'apps/desktop/tests/native/general/native-context.ts',SOURCE_BUILD_HELPER,
 'apps/desktop/src-tauri/src/main.rs','apps/desktop/src-tauri/src/visual_verification.rs',
 'apps/desktop/src-tauri/tauri.visual.conf.json','apps/desktop/src-tauri/tauri.conf.json',
 'apps/desktop/src-tauri/capabilities/default.json','apps/desktop/src-tauri/Cargo.toml','apps/desktop/src-tauri/build.rs',
 'Cargo.toml','Cargo.lock','rust-toolchain.toml','apps/desktop/vite.config.ts','apps/desktop/general-visual.html',
 'apps/desktop/package.json','apps/desktop/package-lock.json','package.json','package-lock.json')


def native_source_binding(root,source_map,sealed_source):
 """Prove an exact S1 renderer closure while collecting with current tools."""
 from current_general_visual_authority import read
 import re
 root=Path(root)
 if not isinstance(sealed_source,str) or not re.fullmatch('[0-9a-f]{64}',sealed_source):raise ValueError('Exact sealed source producer identity required')
 path=artifact(root,source_map)
 if not source_map['path'].startswith('.build/'):raise ValueError('Original managed S1 source map required')
 record=load(path);files=record.get('files')
 if not isinstance(files,dict) or not files or not all(isinstance(k,str) and isinstance(v,str) for k,v in files.items()):raise ValueError('Complete original S1 file identity map required')
 actual=hashlib.sha256(json.dumps(files,sort_keys=True).encode()).hexdigest()
 if actual!=sealed_source or record.get('source_tree_sha256')!=sealed_source or record.get('count')!=len(files) or record.get('source_scope_errors')!=[]:
  raise ValueError('Original S1 source map does not recompute to the sealed package source')
 embedded={}
 for relative in NATIVE_EMBEDDED_INPUTS:
  data=read(relative,root);value=hashlib.sha256(data).hexdigest()
  if files.get(relative)!=value:raise ValueError('S1 embedded Native source/renderer input changed: '+relative)
  embedded[relative]={'sha256':value,'bytes':len(data)}
 current=reference_inputs(root,'packaged_native')
 external={key:value for key,value in current.items() if key not in embedded}
 return {'kind':'general-native-source-producer-binding/v1','source_map':source_map,'sealed_source_tree_sha256':sealed_source,
         'embedded_inputs':embedded,'external_current_collector_inputs':external,
         'epoch_policy':'S1 exact embedded source renderer; external collectors separately bind their current epoch; final S2 candidate must bind its own complete source/archive',
         'product_accepted':False}
