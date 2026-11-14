#!/usr/bin/env python3
"""Read-only disposition of one immutable user-accepted General Native batch.

This never captures UI, alters a comparator, or promotes game/storage/release
acceptance. The retained zero-RGBA failures remain original failed evidence.
"""
from __future__ import annotations
import argparse
import copy
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import secrets
import shutil
import tempfile

from gate_core import safe_path, source_files
from current_general_visual_authority import capture_cases, case_key, STATE_IDS, SCOPE, authority, context as visual_context
from current_general_visual_inspect import check_facts, native_process_binding, controller_runtime_identity, inspect_native_settle, _micros
from current_general_visual_recompute import pixels, compare_semantics

ROOT = Path(__file__).resolve().parents[2]
RECORD_PATH = 'docs/development/general-native-visual-acceptance-20261109.json'
# The reviewed complete record is code registered. JSON/CLI cannot select policy.
RECORD_SHA256 = '0cf2d6134e2b043b8555639c23a5a3f501835bcf5f159659976ea11eb8394e12'
ACCEPTED_ARTIFACT_SHA256 = '12142ece8662631ece70c112d83e69b7a5ea006fc4f2fe54e4432886da0c70fe'
CANDIDATE_SOURCE_SHA256 = '62c348364f3e4337ad344e34fb0ff775b085b4d63f89aa9c3c4d8dec63676f9d'
REVIEWED_TOOLING_CHANGE = 'close-engineering-gate-drift'
REVIEWED_TOOLING_BASE_SOURCE_SHA256 = 'eb54b177de744d3c2f0e09050abd6899919d83781f9cf3af400b78db580a6723'
REVIEWED_TOOLING_PATHS_SHA256 = '89b52cf07819e8bbe6786d27b3b26993cb27cbe5f0db2a6c8de984e245941d50'
DECISION_PATH = '.build/general-visual-closure-20261108/phase-2/general-native-visual-user-decision.json'
DECISION_SHA256 = '72ebb5865b3776e32c8d43f5b833d09a0dc6187e5e2adf691ad51bde087be36e'
CHECK_ID = 'general-native-visual-acceptance'
ADAPTER_ID = 'product-general-visual-native-tests'
COMMAND = ('python3', 'scripts/quality/current_general_visual_acceptance.py', '--check')
KIND = 'general-native-scoped-user-acceptance-evaluation/v1'
RECEIPT_KIND = 'general-native-scoped-user-acceptance-receipt/v1'
ALLOWED_CURRENT_DELTA_PATHS = (
 'scripts/quality/current_general_visual_acceptance.py',
 'scripts/quality/test_current_general_visual_acceptance.py',
 'scripts/quality/current_design_alignment.py', 'scripts/quality/verify.py', RECORD_PATH,
 'docs/development/design-alignment-and-release.md', 'docs/development/engineering-gates.md', 'docs/README.md',
 *('openspec/changes/consume-scoped-visual-acceptance/'+name for name in
   ('proposal.md','design.md','tasks.md','iteration-card.md','verification.json','source-snapshot.json','specs/visual-acceptance/spec.md')),
)
ORIGINAL_KEYS = frozenset(('root_receipt','native_report','native_receipt','native_validation','native_request',
 'native_execution','react_report','react_receipt','react_validation','reference_catalogue','reference_review','reference_generation'))
SHA = re.compile(r'[0-9a-f]{64}\Z')
NONCE = re.compile(r'[0-9a-f]{32}\Z')


def now():
 return datetime.now(timezone.utc).isoformat()


def load(path):
 """Strict JSON for retained evidence, including overflowing float literals."""
 def unique(pairs):
  value={}
  for key,item in pairs:
   if key in value: raise ValueError('Duplicate retained JSON key: '+key)
   value[key]=item
  return value
 def floating(text):
  value=float(text)
  if not math.isfinite(value): raise ValueError('Nonfinite retained JSON number')
  return value
 def constant(text): raise ValueError('Nonfinite retained JSON constant: '+text)
 return json.loads(Path(path).read_text(encoding='utf-8'),object_pairs_hook=unique,
                   parse_float=floating,parse_constant=constant)


def file_sha(path):
 with Path(path).open('rb') as stream:
  return hashlib.file_digest(stream, 'sha256').hexdigest()


def tree_sha(files):
 return hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()


def is_registered_artifact(value):
 # Never read the decision here: corruption of the registered record must fail
 # in evaluation, rather than silently falling through to a fresh GUI collector.
 return isinstance(value, str) and value == ACCEPTED_ARTIFACT_SHA256


def _ref(root, value):
 if (not isinstance(value, dict) or set(value) != {'path','sha256'}
     or not isinstance(value['sha256'], str) or not SHA.fullmatch(value['sha256'])):
  raise ValueError('Exact retained path/hash reference required')
 path = _path(root, value['path'])
 if not path.is_file() or file_sha(path) != value['sha256']:
  raise ValueError('Retained original missing or hash changed: '+value['path'])
 return path


def _path(root, relative):
 if (not isinstance(relative,str) or not relative or relative != relative.strip()
     or any(ch in relative for ch in ('\\',':','?','#','\x00'))
     or any(ord(ch)<32 or ord(ch)==127 for ch in relative)
     or PurePosixPath(relative).is_absolute() or '..' in PurePosixPath(relative).parts
     or PurePosixPath(relative).as_posix()!=relative):
  raise ValueError('Canonical finite relative POSIX evidence path required')
 if Path(root).is_symlink(): raise ValueError('Symlink repository cannot supply retained evidence')
 return safe_path(Path(root),relative)


def _record(root):
 path = _ref(root, {'path':RECORD_PATH,'sha256':RECORD_SHA256})
 value = load(path)
 expected = {'kind':'general-native-scoped-visual-acceptance/v1','status':'accepted_by_user',
             'scope':SCOPE,'candidate_source_tree_sha256':CANDIDATE_SOURCE_SHA256}
 for key, wanted in expected.items():
  if value.get(key) != wanted: raise ValueError('Registered user acceptance identity differs: '+key)
 if type(value.get('schema_version')) is not int or value['schema_version']!=1:
  raise ValueError('Typed current acceptance record schema required')
 if value.get('allowed_current_delta_paths') != list(ALLOWED_CURRENT_DELTA_PATHS):
  raise ValueError('Acceptance cannot enlarge the fixed QA/document source delta')
 if value.get('decision') != {'path':DECISION_PATH,'sha256':DECISION_SHA256}:
  raise ValueError('Original explicit user decision differs')
 decision = load(_ref(root,value['decision']))
 if (decision.get('kind') != 'explicit-scoped-native-visual-user-decision'
     or decision.get('status') != 'accepted_by_user_with_retained_bit_exact_failure'
     or decision.get('artifact_sha256') != ACCEPTED_ARTIFACT_SHA256
     or decision.get('source_tree_sha256') != CANDIDATE_SOURCE_SHA256
     or type(decision.get('raw_comparison_threshold')) is not int or decision['raw_comparison_threshold'] != 0):
  raise ValueError('User disposition must retain its original failed bit-exact result')
 for key in ('approved_source_or_reference_modified','accepts_missing_content_or_layout_errors',
             'functionality_or_interaction_acceptance_promoted','full_product_acceptance_promoted',
             'future_changed_source_or_artifact_automatically_accepted'):
  if decision.get(key) is not False: raise ValueError('User acceptance exceeds its finite visual scope: '+key)
 return value, decision


def _tooling_increment_path(relative):
 """A reviewed verifier revision cannot authorize new production/design bytes."""
 if (not isinstance(relative, str) or not relative or relative!=relative.strip()
     or any(c in relative for c in '*?[]{}\\:#')
     or any(ord(c)<32 or ord(c)==127 for c in relative)
     or PurePosixPath(relative).is_absolute()
     or PurePosixPath(relative).as_posix()!=relative
     or any(p in ('','.','..') for p in relative.split('/'))):
  return False
 if relative.startswith(('docs/development/design-approval-',
                         'docs/development/current-general-reference-',
                         'docs/development/current-reference-')):
  return False
 return (relative == 'docs/README.md'
         or relative.startswith(('scripts/quality/', 'docs/architecture/', 'docs/development/',
                                 'openspec/changes/'+REVIEWED_TOOLING_CHANGE+'/')))


def _reviewed_tooling_increment(root, candidate_before, current):
 """Require the new change's real non-author whole-tree review, not a pass flag.

 The original accepted record and its fifteen paths stay immutable. An extra
 verifier-only change is separately rooted in the previously reviewed EB54
 snapshot and cannot inherit an approval for modified production source.
 """
 from gate_core import check_scope
 from spec_contract import validate_change
 from change_review import validate_review
 prefix='openspec/changes/'+REVIEWED_TOOLING_CHANGE+'/'
 metadata=load(_path(root,prefix+'verification.json'))
 allowed=metadata.get('allowed_paths')
 if (metadata.get('kind')!='tooling' or metadata.get('id')!=REVIEWED_TOOLING_CHANGE
     or metadata.get('source_snapshot')!=prefix+'source-snapshot.json'
     or not isinstance(allowed,list) or not allowed
     or any(not _tooling_increment_path(p) for p in allowed)
     or len(allowed)!=36 or len(allowed)!=len(set(allowed))
     or hashlib.sha256(json.dumps(allowed,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()!=REVIEWED_TOOLING_PATHS_SHA256):
  raise ValueError('Only the exact reviewed tooling-only change may extend verifier inputs')
 snapshot=load(_path(root,metadata['source_snapshot']))
 parent=snapshot.get('files')
 if (not isinstance(parent,dict) or not parent
     or snapshot.get('source_tree_sha256')!=REVIEWED_TOOLING_BASE_SOURCE_SHA256
     or tree_sha(parent)!=REVIEWED_TOOLING_BASE_SOURCE_SHA256):
  raise ValueError('Reviewed verifier increment must use the immutable EB54 before-image')
 for relative in candidate_before.keys() | parent.keys():
  if (relative not in ALLOWED_CURRENT_DELTA_PATHS
      and candidate_before.get(relative)!=parent.get(relative)):
   raise ValueError('Verifier parent differs from the accepted production source')
 changes={p for p in parent.keys() | current.keys() if parent.get(p)!=current.get(p)}
 if not changes <= set(allowed):
  raise ValueError('Current verifier changes exceed the reviewed exact scope')
 errors=validate_change(root,REVIEWED_TOOLING_CHANGE)
 paths,scope_errors=check_scope(root,metadata)
 if errors or scope_errors:
  raise ValueError('Reviewed verifier change contract or scope is invalid: '+str(errors+scope_errors))
 if not changes:
  raise ValueError('Reviewed verifier increment requires its complete nonempty frozen-source delta')
 review=validate_review(root,REVIEWED_TOOLING_CHANGE,metadata,sorted(changes))
 if review.get('status')!='passed' or review.get('errors'):
  raise ValueError('Current verifier increment lacks an independent exact whole-source review: '+str(review.get('errors')))
 return set(allowed)


def _current_source(root, record):
 snapshot = load(_ref(root,record['source_snapshot']))
 before = snapshot.get('files')
 if (not isinstance(before,dict) or not before or snapshot.get('source_tree_sha256') != CANDIDATE_SOURCE_SHA256
     or tree_sha(before) != CANDIDATE_SOURCE_SHA256):
  raise ValueError('Frozen candidate source snapshot does not recompute to source62')
 current = source_files(root)
 extra={p for p in before.keys() | current.keys()
        if p not in ALLOWED_CURRENT_DELTA_PATHS and before.get(p)!=current.get(p)}
 allowed=set(ALLOWED_CURRENT_DELTA_PATHS)
 if extra:
  try:
   allowed.update(_reviewed_tooling_increment(root,before,current))
  except (OSError,ValueError,KeyError,TypeError,AttributeError) as error:
   raise ValueError('Current source outside accepted QA/document delta changed: '
                    +sorted(extra)[0]+'; reviewed increment refused: '+str(error)) from error
 for relative in before.keys() | current.keys():
  _path(root,relative)
  if relative not in allowed and before.get(relative) != current.get(relative):
   raise ValueError('Current source outside accepted QA/document delta changed: '+relative)
 if any(value.startswith('symlink:') for value in current.values()):
  raise ValueError('Symlink cannot inherit retained candidate acceptance')
 return tree_sha(current)


def _immutable(root, record):
 rows = record.get('immutable_files')
 if not isinstance(rows,list) or not rows: raise ValueError('Complete retained original closure absent')
 found = {}
 for row in rows:
  if not isinstance(row,dict) or set(row) != {'path','sha256','bytes'} or row['path'] in found:
   raise ValueError('Duplicate or malformed retained original closure')
  path = _ref(root, {key:row[key] for key in ('path','sha256')})
  if type(row['bytes']) is not int or path.stat().st_size != row['bytes']:
   raise ValueError('Retained original size changed: '+row['path'])
  # Parsing every retained JSON rejects duplicate keys and nonfinite values,
  # including otherwise unconsumed auxiliary raw display/OS evidence.
  if path.suffix == '.json': load(path)
  found[row['path']] = row
 return found


class _Batch:
 def __init__(self):
  self.packages = {}
  self.controllers = {}
  self.retained = None


def _inspect_package_batch(root, package, batch):
 """One actual signature/archive inspection per unique immutable archive."""
 from package_identity import inspect_package, read_signature_metadata
 root=Path(root)
 app,binary,archive=(Path(package[key]) for key in ('app','binary','archive'))
 for path in (app,binary,archive):
  if path != path.resolve() or not path.is_relative_to(root/'.build'):
   raise ValueError('Retained Native package path is not canonical managed build')
  safe_path(root,path.relative_to(root).as_posix())
 if app != archive.parent/'Luna.app' or binary != app/'Contents/MacOS/luna-reader':
  raise ValueError('Retained Native app/binary/archive are not one package')
 identity = (str(archive), package['archive_sha256'],package['source_tree_sha256'])
 if identity in batch.packages:
  if batch.packages[identity]['package'] != package: raise ValueError('Same archive has inconsistent retained package identity')
  return batch.packages[identity]['signature']
 bindings = ((binary,'executable_sha256'),(archive,'archive_sha256'),
             (app/'Contents/Resources/luna-source-identity.json','sealed_identity_sha256'),
             (app.parent/'build-manifest.json','external_manifest_sha256'))
 if batch.retained is None or any(path.relative_to(root).as_posix() not in batch.retained for path,_ in bindings):
  raise ValueError('Retained closure must seal every archive/binary/source identity input')
 if any(file_sha(path) != package[key] for path,key in bindings):
  raise ValueError('Retained Native source/archive/executable/seal changed')
 if b'luna-general-native-isolated-renderer/v1:20261108' not in binary.read_bytes():
  raise ValueError('Retained package lacks isolated Native composition')
 with tempfile.TemporaryDirectory(prefix='accepted-native-package-',dir=root/'.build') as tmp:
  inspected = inspect_package(root,archive,package['source_tree_sha256'],Path(tmp))
 signature = read_signature_metadata(app)
 if (signature != package['signature'] or signature.get('mode') != 'certificate'
     or signature.get('identifier') != 'com.lunatranslator.reader'
     or inspected.get('executable_sha256') != package['executable_sha256']
     or inspected.get('archive_sha256') != package['archive_sha256']):
  raise ValueError('Retained Native actual signature/source identity differs')
 batch.packages[identity] = {'package':copy.deepcopy(package),'signature':signature,'inspection':inspected}
 return signature


def _inspect_native_case(root, directory, row, side, batch):
 """Reinspect sealed raw facts, OS cohort and frame provenance without a launch."""
 from current_general_native_display import validate_capture_environment
 from reading_alignment_preflight import png_pixels
 root=Path(root); directory=_path(root,directory)
 r=load(directory/'capture.json'); package=r['package']
 if (r.get('kind')!='general-packaged-native-render-capture/v1' or r.get('status')!='captured'
     or r.get('errors')!=[] or r.get('side')!=side
     or any(r.get(k) is not False for k in ('game_tests_executed','os_runtime_verified','storage_runtime_verified'))):
  raise ValueError('Retained controlled Native original failed or overclaims runtime')
 if any(r.get(k)!=row[k] for k in ('state_id','canvas_id','subcase')) or package.get('source_tree_sha256')!=r.get('source_tree_sha256'):
  raise ValueError('Retained Native case/source identity differs')
 signature=_inspect_package_batch(root,package,batch)
 app,binary=Path(package['app']),Path(package['binary'])
 started,observed,finished=(load(directory/name) for name in ('started.json','observation.json','finished.json'))
 obs=observed['observation'];ctx=obs['context'];nonce=r['execution_nonce'];pid=r['pid'];controller=r['controller_pid'];parent=r['controller_parent_pid']
 if (not isinstance(nonce,str) or not NONCE.fullmatch(nonce) or ctx!=visual_context(row,side,nonce,'packaged_native')
     or started.get('context')!=ctx or any(v.get('pid')!=pid for v in (started,observed,finished))):
  raise ValueError('Retained Native PID/nonce/context differs')
 if (started.get('mode_contract')!='luna-general-native-isolated-renderer/v1:20261108'
     or started.get('business_runtime_constructed') is not False
     or observed.get('kind')!='general-native-raw-observation/v1'
     or observed.get('window_label')!='general-visual-verification' or observed.get('executable')!=str(binary)
     or observed.get('source_identity')!=load(app/'Contents/Resources/luna-source-identity.json')
     or file_sha(directory/'observation.json')!=r['observation_sha256']):
  raise ValueError('Retained Native isolated source/composition/window differs')
 marker=(directory/'finish-marker.txt').read_bytes()
 if (marker!=nonce.encode() or hashlib.sha256(marker).hexdigest()!=r['finish_marker_sha256']
     or finished.get('status')!='finished_by_owned_controller' or r.get('finish_requested') is not True or r.get('exit_code')!=0):
  raise ValueError('Retained Native owned completion absent')
 if r.get('command')!=[str(binary),'--general-visual-verification',side,row['state_id'],row['canvas_id'],row['subcase'],nonce]:
  raise ValueError('Retained Native original launch command differs')
 command=load(directory.with_name(directory.name+'.command.json')); actual=command.get('command',[])
 tail=[str(root/'scripts/report/verify_general_native_visual.py'),'--app',str(app),'--expected-source',r['source_tree_sha256'],
       '--side',side,'--state',row['state_id'],'--canvas',row['canvas_id'],'--subcase',row['subcase'],'--output',str(directory)]
 if len(actual)==len(tail)+1 and actual[1]=='scripts/report/verify_general_native_visual.py': tail[0]=actual[1]
 if actual[1:]!=tail or command.get('exit_code')!=0 or command.get('pid')!=controller or command.get('parent_pid')!=parent or command.get('cwd')!=str(root):
  raise ValueError('Retained Native original Popen/controller binding differs')
 executable=str(Path(shutil.which(actual[0]) or actual[0]).resolve())
 if r.get('controller_executable')!=executable: raise ValueError('Retained Native controller runtime differs')
 if executable not in batch.controllers: batch.controllers[executable]=controller_runtime_identity(executable)
 before,after,exited=(load(directory/name) for name in ('os-before.json','os-after.json','os-exited.json'))
 low,high=_micros(command['started_at']),_micros(command['finished_at'])
 if not low<=_micros(r['started_at'])<=_micros(r['finished_at'])<=high: raise ValueError('Retained Native capture interval differs')
 bindings=[native_process_binding(value,pid,binary,signature,controller,executable,parent,low,high,batch.controllers[executable]) for value in (before,after)]
 if bindings[0]!=bindings[1] or not low<=before['observed_at_ms']*1000<=after['observed_at_ms']*1000<=exited['observed_at_ms']*1000<=high:
  raise ValueError('Retained Native OS cohort/order changed')
 for os in (before,after):
  windows=[w for w in os.get('windows',[]) if w.get('pid')==pid and w.get('window_id')==r['window_id']]
  visible=[w for w in os.get('windows',[]) if w.get('pid')==pid and w.get('on_screen') is True and w.get('layer')==0]
  if len(windows)!=1 or len(visible)!=1 or windows[0]!=visible[0] or windows[0]!=r['window'] or windows[0]['bounds']['Width']!=row['width'] or windows[0]['bounds']['Height']!=row['height']:
   raise ValueError('Retained Native complete visible window geometry differs')
 ended=[p for p in exited.get('processes',[]) if p.get('pid')==pid]
 if (len(ended)!=1 or ended[0].get('bsd_bytes',0)>0 or ended[0].get('path_bytes',0)>0
     or any(w.get('pid')==pid for w in exited.get('windows',[]))
     or (exited['boot_sec'],exited['boot_usec'])!=bindings[0]['boot']):
  raise ValueError('Retained Native clean exit/boot cohort differs')
 scale=observed['window']['backing_scale']; dimensions={'width':row['width']*scale,'height':row['height']*scale}
 if type(scale) not in (int,float) or scale<=0 or observed['window']['inner_pixels']!=dimensions or observed['window']['outer_pixels']!=dimensions:
  raise ValueError('Retained Native logical/inner/outer/backing geometry differs')
 frames=r['frames']
 if len(frames)!=2 or [f['path'] for f in frames]!=['frame-1.png','frame-2.png']: raise ValueError('Retained Native two original frame slots differ')
 inspect_native_settle(r,before,after)
 for frame in frames:
  path=safe_path(directory,frame['path']);w,h,_=png_pixels(path.read_bytes(),decode_pixels=False)
  if (file_sha(path)!=frame['sha256'] or path.stat().st_size!=frame['bytes']
      or [w,h]!=[row['width']*scale,row['height']*scale] or [w,h]!=[frame['pixel_width'],frame['pixel_height']]
      or frame['command']!=['/usr/sbin/screencapture','-x','-a','-o','-l',str(r['window_id']),str(path)]
      or not before['observed_at_ms']*1000<=_micros(frame['captured_at'])<=after['observed_at_ms']*1000):
   raise ValueError('Retained Native full original PNG/DPR/window/time differs')
 validate_capture_environment(root,directory,r,before,after,scale,low,high)
 check_facts(row,obs,side,'packaged_native')
 return [directory/f['path'] for f in frames],obs,r


def _originals(root, record, retained):
 originals=record.get('originals')
 if not isinstance(originals,dict) or set(originals)!=ORIGINAL_KEYS:
  raise ValueError('Complete exact original report/receipt/reference set required')
 values={}
 for name,reference in originals.items():
  path=_ref(root,reference)
  if reference['path'] not in retained: raise ValueError('Original report omitted from retained closure: '+name)
  values[name]=load(path)
 native=values['native_report']; receipt=values['native_receipt']; validation=values['native_validation']
 if (native.get('kind')!='general-packaged-native-execution/v1' or native.get('scope')!=SCOPE
     or native.get('layer')!='packaged_native' or native.get('collection_complete') is not True
     or native.get('source_tree_sha256')!=CANDIDATE_SOURCE_SHA256
     or native.get('artifact_sha256')!=ACCEPTED_ARTIFACT_SHA256
     or native.get('source_authority')!=authority(root) or native.get('context')!=values['native_request']):
  raise ValueError('Original Native execution identity/completeness differs')
 if any(native.get(k) is not False for k in ('game_tests_executed','os_runtime_verified','storage_runtime_verified')):
  raise ValueError('Original Native controlled renderer exceeds authority')
 if (receipt.get('status')!='failed' or receipt.get('adapter_id')!=ADAPTER_ID
     or receipt.get('source_tree_sha256')!=CANDIDATE_SOURCE_SHA256
     or receipt.get('artifact_sha256')!=ACCEPTED_ARTIFACT_SHA256
     or receipt.get('report')!=originals['native_report']
     or receipt.get('execution')!=originals['native_execution']
     or receipt.get('execution_nonce')!=native.get('execution_nonce')):
  raise ValueError('Original Native failed receipt or invocation changed')
 if validation.get('inspection')!=[]:
  raise ValueError('Nonpixel Native inspection errors cannot be accepted')
 command=values['native_execution']
 if command.get('exit_code')!=0 or command.get('cwd')!=str(Path(root)):
  raise ValueError('Original actual Native command did not complete')
 if not _micros(command['started_at'])<=_micros(native['started_at'])<=_micros(native['finished_at'])<=_micros(command['finished_at']):
  raise ValueError('Original Native command/capture interval differs')
 root_receipt=values['root_receipt']
 if root_receipt.get('status')!='failed' or root_receipt.get('source_tree_sha256')!=CANDIDATE_SOURCE_SHA256:
  raise ValueError('Original Root failed result must remain unchanged')
 steps=root_receipt.get('steps',[])
 for identity,status in ((ADAPTER_ID,'failed'),('product-general-visual-react-tests','passed')):
  found=[v for v in steps if v.get('id')==identity]
  if len(found)!=1 or found[0].get('status')!=status:
   raise ValueError('Original Root Native/React outcome differs')
 react=values['react_receipt']; report=values['react_report']; react_validation=values['react_validation']
 if (react.get('status')!='passed' or react.get('errors')!=[] or react.get('layer')!='real_react'
     or react.get('artifact_sha256')!=ACCEPTED_ARTIFACT_SHA256
     or react.get('source_tree_sha256')!=CANDIDATE_SOURCE_SHA256
     or react.get('report')!=originals['react_report'] or report.get('collection_complete') is not True
     or [v.get('case') for v in report.get('observations',[])]!=list(capture_cases())
     or react_validation.get('inspection')!=[] or react_validation.get('independent_recomputation')!=[]):
  raise ValueError('Retained React27 result differs')
 catalogue=values['reference_catalogue']; review=values['reference_review']; generation=values['reference_generation']
 if (catalogue.get('kind')!='general-independent-reference-catalogue/v1' or catalogue.get('scope')!=SCOPE
     or catalogue.get('source_authority')!=authority(root)
     or review.get('kind')!='general-independent-reference-review/v1' or review.get('status')!='approved'
     or review.get('catalogue')!=originals['reference_catalogue']
     or catalogue.get('layers',{}).get('packaged_native',{}).get('generation')!=originals['reference_generation']
     or generation.get('kind')!='general-reference-candidates/v1' or generation.get('layer')!='packaged_native'
     or generation.get('authority')!=authority(root)):
  raise ValueError('Independent approved reference closure changed')
 return values


def _recompute(root, record, values, retained, batch):
 from current_general_native_display import validate_generation_environment
 expected=list(capture_cases()); rows=record.get('observations',[])
 if [r.get('case') for r in rows]!=expected: raise ValueError('Accepted fixed27 case closure omitted/duplicated/reordered')
 native_rows=values['native_report'].get('observations',[])
 source_rows=values['reference_generation'].get('captures',[])
 catalogue_rows=values['reference_catalogue']['layers']['packaged_native']['entries']
 if ([r.get('case') for r in native_rows]!=expected or [r.get('case') for r in source_rows]!=expected
     or len(catalogue_rows)!=27 or {r.get('case_key') for r in catalogue_rows}!={case_key(r) for r in expected}):
  raise ValueError('Original candidate/source/catalogue fixed27 closure differs')
 catalogue={r['case_key']:r for r in catalogue_rows}
 validate_generation_environment(root,native_rows,'candidate')
 validate_generation_environment(root,source_rows,'reference')
 comparisons=[]; errors=[]; nonces=set(); raw_directories=set()
 for accepted,candidate,reference in zip(rows,native_rows,source_rows):
  case=accepted['case']; key=case_key(case); cat=catalogue[key]
  cdir=accepted['candidate_raw_directory']; rdir=accepted['reference_raw_directory']
  if cdir!=candidate.get('raw_directory') or rdir!=reference.get('raw_directory') or cat.get('case')!=case:
   raise ValueError('Accepted raw source/candidate directory/case substitution')
  if cat.get('png',{}).get('path')!=rdir+'/frame-1.png' or cat.get('facts',{}).get('path')!=rdir+'/observation.json':
   raise ValueError('Independent catalogue frame/facts substituted')
  for name in ('png','facts'):
   _ref(root,cat[name])
   reviewed='reviewed_png_sha256' if name=='png' else 'reviewed_facts_sha256'
   if values['reference_review'].get(reviewed,{}).get(cat[name]['path'])!=cat[name]['sha256']:
    raise ValueError('Independent original frame/facts not explicitly reviewed')
  for directory in (cdir,rdir):
   if directory in raw_directories: raise ValueError('Raw Native case directory reused')
   raw_directories.add(directory)
   path=_path(root,directory)
   for original in path.rglob('*'):
    relative=original.relative_to(root).as_posix();_path(root,relative)
    if original.is_file() and relative not in retained: raise ValueError('Raw original absent from complete immutable closure: '+relative)
   command=directory+'.command.json'
   if command not in retained: raise ValueError('Original owned controller sidecar absent from closure')
  candidate_frames,candidate_facts,capture=_inspect_native_case(root,cdir,case,'candidate',batch)
  reference_frames,reference_facts,source_capture=_inspect_native_case(root,rdir,case,'reference',batch)
  if capture.get('source_tree_sha256')!=CANDIDATE_SOURCE_SHA256 or capture.get('package',{}).get('archive_sha256')!=ACCEPTED_ARTIFACT_SHA256:
   raise ValueError('Retained candidate came from a different source/archive')
  for observation,side in ((reference_facts,'reference'),(candidate_facts,'candidate')):
   check_facts(case,observation,side,'packaged_native')
   nonce=observation['context']['nonce']
   if nonce in nonces: raise ValueError('Original Native source/candidate nonce reused')
   nonces.add(nonce)
  compare_semantics(reference_facts,candidate_facts)
  if len(reference_frames)!=2 or pixels(reference_frames[0],reference_frames[1])['different_rgba_pixels']!=0:
   raise ValueError('Independent Native source self-calibration nonzero')
  measured=[pixels(reference_frames[0],frame) for frame in candidate_frames]
  if len(measured)!=2 or measured!=accepted.get('comparisons'):
   raise ValueError('Original54 PNG pixel dimensions/differences changed')
  for frame,difference in zip(candidate_frames,measured):
   if difference['different_rgba_pixels']!=0:
    errors.append(f'General original packaged_native {key}: {difference["different_rgba_pixels"]} RGBA pixels differ (required 0)')
   comparisons.append({'case_key':key,'frame':frame.name,**difference})
 if len(comparisons)!=54 or len(errors)!=54:
  raise ValueError('Accepted original54 nonzero pixel failure closure differs')
 if errors!=values['native_validation'].get('independent_recomputation') or errors!=values['native_receipt'].get('errors'):
  raise ValueError('Recomputed original pixel error list is not the exact retained Native failure')
 return comparisons,errors


def _evaluate(root, artifact=None, *, context=None, batch=None):
 root=Path(root).resolve();started=now();nonce=secrets.token_hex(16) if context is None else context.get('execution_nonce')
 result={'schema_version':1,'kind':KIND,'status':'failed','errors':[],'scope':SCOPE,
         'acceptance_status':'not_accepted','raw_bit_exact_status':'failed_retained_unchanged',
         'execution_nonce':nonce,'started_at':started,'finished_at':None,
         'candidate_source_tree_sha256':CANDIDATE_SOURCE_SHA256,'artifact_sha256':ACCEPTED_ARTIFACT_SHA256,
         'decision_record':{'path':RECORD_PATH,'sha256':RECORD_SHA256},
         'command':list(COMMAND),'new_product_capture_executed':False,
         'product_runtime_tests_executed':False,'product_alignment_approved':False,
         'product_component_tests_executed':False,'product_packaged_native_visual_tests_executed':False,
         'product_packaged_native_game_tests_executed':False,'product_storage_tests_executed':False,
         'os_runtime_verified':False,'storage_runtime_verified':False,'game_tests_executed':False,
         'native_verified':False,'full_product_acceptance_promoted':False,'release_acceptance_promoted':False,
         'global_product_gate_passed':False}
 try:
  if not isinstance(nonce,str) or not NONCE.fullmatch(nonce): raise ValueError('Fresh typed evaluator nonce required')
  record,decision=_record(root);source=_current_source(root,record)
  result['source_tree_sha256']=source
  result['evaluator_source']={'path':'scripts/quality/current_general_visual_acceptance.py',
                              'sha256':file_sha(safe_path(root,'scripts/quality/current_general_visual_acceptance.py'))}
  registered=_ref(root,record['artifact'])
  if record['artifact']['sha256']!=ACCEPTED_ARTIFACT_SHA256: raise ValueError('Registered accepted archive differs')
  package=registered if artifact is None else _path(root,artifact)
  if not package.is_file() or file_sha(package)!=ACCEPTED_ARTIFACT_SHA256: raise ValueError('Only exact accepted artifact hash is eligible')
  if context is not None and (context.get('source_tree_sha256')!=source or context.get('artifact_sha256')!=ACCEPTED_ARTIFACT_SHA256
      or context.get('adapter_id')!=ADAPTER_ID or context.get('command')!=list(COMMAND)):
   raise ValueError('Fresh evaluator invocation/source/package/command differs')
  retained=_immutable(root,record);values=_originals(root,record,retained)
  if nonce == values['native_report'].get('execution_nonce'):
   raise ValueError('Original capture nonce cannot become a fresh evaluator execution')
  active_batch=batch or _Batch();active_batch.retained=retained
  comparisons,raw_errors=_recompute(root,record,values,retained,active_batch)
  if _current_source(root,record)!=source: raise ValueError('Source changed during retained acceptance evaluation')
  _immutable(root,record)
  result.update(status='passed',acceptance_status='accepted_by_user',comparison_count=len(comparisons),
                comparisons=comparisons,retained_raw_errors=raw_errors,originals=record['originals'],
                original_execution_nonce=values['native_report']['execution_nonce'],
                retained_original_count=len(retained),source_snapshot=record['source_snapshot'],
                package_inspections=[value['inspection'] for _,value in sorted(active_batch.packages.items())])
 except (OSError,ValueError,KeyError,TypeError,AttributeError,ImportError) as error:
  result['errors'].append('Scoped user visual acceptance refused: '+str(error))
 result['finished_at']=now()
 return result


def evaluate(root=ROOT, artifact=None, *, context=None):
 return _evaluate(root,artifact,context=context)


def _write(path,value):
 with Path(path).open('x',encoding='utf-8') as stream:
  stream.write(json.dumps(value,ensure_ascii=False,indent=2)+'\n')


def _execute(root,context,batch):
 root=Path(root);output=_path(root,context['output'])
 if load(output/'request.json')!=context: raise ValueError('Actual read-only evaluator request differs')
 report=_evaluate(root,context['artifact_path'],context=context,batch=batch)
 path=output/'acceptance-evaluation.json';_write(path,report)
 receipt={key:context[key] for key in ('schema_version','adapter_id','target_id','composition_id','layer',
          'design_closure_sha256','source_tree_sha256','artifact_sha256','command','execution_nonce','state_ids')}
 receipt.update(kind=RECEIPT_KIND,status=report['status'],errors=report['errors'],scope=SCOPE,
   acceptance_status=report['acceptance_status'],candidate_source_tree_sha256=CANDIDATE_SOURCE_SHA256,
   started_at=report['started_at'],finished_at=report['finished_at'],
   producer_kind='retained_user_acceptance_evaluator',prototype_substitution=False,native_verified=False,
   assertion_count=report.get('comparison_count',0),skipped_assertions=0,
   report={'path':path.relative_to(root).as_posix(),'sha256':file_sha(path)},
   new_product_capture_executed=False,product_packaged_native_visual_tests_executed=False,
   product_component_tests_executed=False,product_runtime_tests_executed=False,
   product_packaged_native_game_tests_executed=False,product_storage_tests_executed=False,
   game_tests_executed=False,os_runtime_verified=False,storage_runtime_verified=False,
   product_alignment_approved=False,full_product_acceptance_promoted=False,
   release_acceptance_promoted=False,global_product_gate_passed=False,
   raw_bit_exact_status='failed_retained_unchanged',decision_record=report['decision_record'])
 return receipt


def execute(root, context):
 return _execute(Path(root),context,_Batch())


def _inspect_receipt(root, receipt, artifact_sha256, context=None, batch=None):
 from current_design_alignment import TARGET,COMPOSITION_ID,DESIGN_CLOSURE_SHA256
 if not is_registered_artifact(artifact_sha256): raise ValueError('User visual acceptance cannot apply to future/changed artifacts')
 expected={'schema_version':1,'kind':RECEIPT_KIND,'adapter_id':ADAPTER_ID,'target_id':TARGET,
           'composition_id':COMPOSITION_ID,'layer':'packaged_native','scope':SCOPE,'status':'passed','errors':[],
           'design_closure_sha256':DESIGN_CLOSURE_SHA256,'artifact_sha256':artifact_sha256,
           'source_tree_sha256':tree_sha(source_files(Path(root))), 'candidate_source_tree_sha256':CANDIDATE_SOURCE_SHA256,
           'command':list(COMMAND),'state_ids':sorted(STATE_IDS),'acceptance_status':'accepted_by_user',
           'producer_kind':'retained_user_acceptance_evaluator','native_verified':False,'prototype_substitution':False,
           'new_product_capture_executed':False,'product_packaged_native_visual_tests_executed':False,
           'product_component_tests_executed':False,'product_runtime_tests_executed':False,
           'product_packaged_native_game_tests_executed':False,'product_storage_tests_executed':False,
           'game_tests_executed':False,'os_runtime_verified':False,'storage_runtime_verified':False,
           'product_alignment_approved':False,'full_product_acceptance_promoted':False,
           'release_acceptance_promoted':False,'global_product_gate_passed':False,
           'raw_bit_exact_status':'failed_retained_unchanged','decision_record':{'path':RECORD_PATH,'sha256':RECORD_SHA256},
           'assertion_count':54,'skipped_assertions':0}
 for key,value in expected.items():
  if type(receipt.get(key)) is not type(value) or receipt.get(key)!=value:
   raise ValueError('Retained acceptance receipt identity differs: '+key)
 nonce=receipt.get('execution_nonce')
 if not isinstance(nonce,str) or not NONCE.fullmatch(nonce): raise ValueError('Fresh acceptance evaluation nonce required')
 started,finished=_micros(receipt['started_at']),_micros(receipt['finished_at'])
 if started>finished: raise ValueError('Actual acceptance evaluation interval differs')
 report_path=_ref(root,receipt['report']);report=load(report_path)
 if context is None:
  # Later read-only consumers must recover the actual dispatcher invocation,
  # rather than accepting a self-labelled nonce/time from a standalone JSON.
  context=load(report_path.parent/'invocation.json')
  request=load(report_path.parent/'request.json')
  if request!={key:value for key,value in context.items() if key!='finished_at'}:
   raise ValueError('Retained evaluator actual request/invocation differ')
 for key in ('status','errors','execution_nonce','started_at','finished_at','source_tree_sha256','artifact_sha256',
             'candidate_source_tree_sha256','acceptance_status','decision_record','command','native_verified',
             'new_product_capture_executed','product_packaged_native_visual_tests_executed','raw_bit_exact_status',
             'product_component_tests_executed','product_runtime_tests_executed','product_storage_tests_executed',
             'product_packaged_native_game_tests_executed','game_tests_executed','os_runtime_verified','storage_runtime_verified',
             'product_alignment_approved','full_product_acceptance_promoted','release_acceptance_promoted','global_product_gate_passed'):
  if type(report.get(key)) is not type(receipt.get(key)) or report.get(key)!=receipt.get(key):
   raise ValueError('Acceptance report/receipt differs: '+key)
 if report.get('kind')!=KIND or report.get('original_execution_nonce')==nonce:
  raise ValueError('Original capture cannot be relabelled as fresh acceptance evaluation')
 if context is not None:
  for key in ('schema_version','adapter_id','target_id','composition_id','layer','design_closure_sha256',
              'source_tree_sha256','artifact_sha256','command','execution_nonce','state_ids'):
   if type(context.get(key)) is not type(receipt.get(key)) or context.get(key)!=receipt.get(key):
    raise ValueError('Actual evaluator invocation differs: '+key)
  if not _micros(context['started_at'])<=started<=finished<=_micros(context['finished_at']):
   raise ValueError('Acceptance evaluation lies outside actual dispatcher interval')
  if _micros(context['finished_at'])>_micros(now()):
   raise ValueError('Actual evaluator invocation cannot finish in the future')
  if receipt['report']['path']!=context['output']+'/acceptance-evaluation.json':
   raise ValueError('Acceptance report not from actual fresh evaluator output')
  if file_sha(safe_path(root,context['artifact_path']))!=artifact_sha256:
   raise ValueError('Actual evaluator artifact changed')
 independent=_evaluate(root,context['artifact_path'] if context else None,context=context,batch=batch)
 if independent['status']!='passed' or independent['errors']:
  raise ValueError('Independent retained acceptance reinspection failed: '+str(independent['errors']))
 for key in ('source_tree_sha256','evaluator_source','candidate_source_tree_sha256','artifact_sha256',
             'acceptance_status','decision_record','comparisons','retained_raw_errors','originals','original_execution_nonce',
             'retained_original_count','source_snapshot','package_inspections'):
  if report.get(key)!=independent.get(key): raise ValueError('Independent retained acceptance recomputation differs: '+key)
 return 'packaged_native',frozenset(STATE_IDS)


def inspect_receipt(root, receipt, artifact_sha256, context=None):
 return _inspect_receipt(Path(root),receipt,artifact_sha256,context)


def dispatch(root, output, artifact=None, *, lifecycle_context=None):
 from current_design_alignment import TARGET,COMPOSITION_ID,DESIGN_CLOSURE_SHA256
 root=Path(root).resolve()
 result={'status':'failed','errors':[],'adapter_id':ADAPTER_ID,'scope':SCOPE,
         'global_product_gate_passed':False,'product_component_tests_executed':False,
         'product_runtime_tests_executed':False,'product_alignment_approved':False,
         'full_product_acceptance_promoted':False,'release_acceptance_promoted':False,
         'product_packaged_native_game_tests_executed':False,'product_packaged_native_visual_tests_executed':False,
         'product_storage_tests_executed':False,'new_product_capture_executed':False}
 try:
  if not isinstance(output,str) or not output.startswith('.build/'): raise ValueError('Fresh .build evaluator directory required')
  destination=_path(root,output)
  if destination.exists(): raise ValueError('Evaluator output exists; old receipt cannot become a fresh invocation')
  record,_=_record(root);package=_ref(root,record['artifact']) if artifact is None else _path(root,artifact)
  if not package.is_file() or file_sha(package)!=ACCEPTED_ARTIFACT_SHA256:
   raise ValueError('Exact user-accepted package hash required')
  context={'schema_version':1,'adapter_id':ADAPTER_ID,'target_id':TARGET,'composition_id':COMPOSITION_ID,
    'layer':'packaged_native','state_ids':sorted(STATE_IDS),'execution_nonce':secrets.token_hex(16),
    'source_tree_sha256':tree_sha(source_files(root)),'design_closure_sha256':DESIGN_CLOSURE_SHA256,
    'artifact_sha256':ACCEPTED_ARTIFACT_SHA256,'artifact_path':package.relative_to(root).as_posix(),
    'command':list(COMMAND),'output':output,'started_at':now()}
  if lifecycle_context is not None:
   nonce=lifecycle_context.get('execution_nonce');start=lifecycle_context.get('started_at')
   if not isinstance(nonce,str) or not NONCE.fullmatch(nonce) or _micros(start)>_micros(context['started_at']):
    raise ValueError('Typed earlier lifecycle invocation required')
   context.update(lifecycle_execution_nonce=nonce,lifecycle_started_at=start)
  destination.mkdir(parents=True);_write(destination/'request.json',context)
  batch=_Batch();receipt=_execute(root,copy.deepcopy(context),batch);context['finished_at']=now()
  _inspect_receipt(root,receipt,ACCEPTED_ARTIFACT_SHA256,context,batch)
  if load(destination/'request.json')!={k:v for k,v in context.items() if k!='finished_at'}:
   raise ValueError('Read-only evaluator rewrote dispatcher request')
  _write(destination/'receipt.json',receipt);_write(destination/'invocation.json',context)
  result.update(status='passed',acceptance_status='accepted_by_user',raw_bit_exact_status='failed_retained_unchanged',
    receipt=output+'/receipt.json',receipt_sha256=file_sha(destination/'receipt.json'),
    invocation=output+'/invocation.json',invocation_sha256=file_sha(destination/'invocation.json'),
    execution_nonce=context['execution_nonce'],candidate_source_tree_sha256=CANDIDATE_SOURCE_SHA256,
    evaluator_source_tree_sha256=context['source_tree_sha256'])
 except (OSError,ValueError,KeyError,TypeError,AttributeError) as error:
  result['errors'].append('Read-only scoped acceptance dispatch failed: '+str(error))
 return result


def main(argv=None):
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--check',action='store_true',required=True)
 parser.add_argument('--output',required=True)
 parser.add_argument('--artifact')
 parser.add_argument('--json',action='store_true')
 args=parser.parse_args(argv)
 result=dispatch(ROOT,args.output,args.artifact)
 print(json.dumps(result,ensure_ascii=False,indent=2))
 return 0 if result['status']=='passed' else 1


if __name__=='__main__': raise SystemExit(main())
