"""Independent original-PNG/DOM recomputation; no producer pass labels or Pillow."""
from __future__ import annotations
from functools import lru_cache
from pathlib import Path
from gate_core import safe_path,digest
from reading_alignment_preflight import png_pixels
from current_general_visual_authority import capture_cases,case_key
from current_general_reference import artifact,load,entries
from current_general_visual_inspect import raw_capture,check_facts


PNG_COMPARISON_CACHE_MAX_BYTES=2*1024*1024


def _png_comparison(left,right):
 aw,ah,a=png_pixels(left);bw,bh,b=png_pixels(right)
 if (aw,ah)!=(bw,bh):raise ValueError('Original unscaled full-canvas PNG dimensions differ')
 if aw>4096 or ah>4096:raise ValueError('Original PNG exceeds bounded whole canvas')
 different=sum(a[i:i+4]!=b[i:i+4] for i in range(0,len(a),4))
 return different,aw,ah


@lru_cache(maxsize=128)
def _cached_png_comparison(left,right):
 # Only complete immutable PNG bytes form the key. Exceptions are not cached.
 return _png_comparison(left,right)


def pixels(left,right):
 # Reread both original files every time, even at the same path/mtime. Limit
 # retained key payload to128*2MiB; larger originals keep identical uncached
 # validation/comparison semantics, including corrupt identical byte streams.
 a,b=Path(left).read_bytes(),Path(right).read_bytes()
 compare=_cached_png_comparison if len(a)+len(b)<=PNG_COMPARISON_CACHE_MAX_BYTES else _png_comparison
 different,width,height=compare(a,b)
 return {'different_rgba_pixels':different,'width':width,'height':height,'threshold':0,'mask':False,'crop':False,'resampled':False}


def reference_observation(root,row,layer):
 p=artifact(root,row['facts']);value=load(p)
 if layer=='real_react':
  if value.get('kind')!='general-webkit-observer-capture/v1' or value.get('mode')!='general-reference' or len(value.get('captures',[]))!=4:raise ValueError('Original independent WK reference source absent')
  # Resolve the fixed original driver result, not an arbitrary caller-provided
  # file. The source path gets the same request/CSP/window/resource inspection
  # as actual React captures before any PNG can become a reference.
  result_file=p.parent.parent/'result.json'
  original,_observation,_execution=raw_capture(root,{'case':row['case'],'result_file':result_file.relative_to(root).as_posix()},layer,'reference')
  if original.parent!=p.parent:raise ValueError('Independent source facts came from a different original capture slot')
  observations=[v['facts']['observation'] for v in value['captures']]
  images=[p.parent/(v['name']+'.png') for v in value['captures']]
  for v,image in zip(value['captures'],images):
   if digest(image)!=v['png_sha256']:raise ValueError('Original reference PNG capture binding changed')
 else:
  original,observation,capture=raw_capture(root,{'case':row['case'],'raw_directory':p.parent.relative_to(root).as_posix()},layer,'reference')
  if p!=original.parent/'observation.json':raise ValueError('Native source facts substituted')
  observations=[observation]
  images=[original.parent/f['path'] for f in capture['frames']]
 for obs in observations:check_facts(row['case'],obs,'reference',layer)
 if artifact(root,row['png'])!=images[0]:raise ValueError('Catalogue PNG is not the predeclared first original frame')
 for image in images[1:]:
  if pixels(images[0],image)['different_rgba_pixels']!=0:raise ValueError('Reference observer self/cross-document calibration nonzero')
 return images[0],observations[0]


def compare_semantics(reference,candidate):
 # Compare user-visible data without equating source in-memory scope IDs to
 # database keys or source saved fixtures to real persistence authority.
 rf,cf=reference['facts'],candidate['facts']
 for k in ('theme','zoom','dirty','menuOpen'):
  if rf.get(k)!=cf.get(k):raise ValueError('Independent user-visible General fact differs: '+k)
 for k in ('kind','disabled','expanded'):
  if rf['scope'].get(k)!=cf['scope'].get(k):raise ValueError('Independent scope semantics differ: '+k)
 lex=('可点击日语分词','显示日语注音','显示词性标记')
 initial=reference['context']['stateId']=='all-page-settings-general-loading' and reference['context']['subcase']=='initial'
 for label in lex:
  a=[r for r in rf['switches'] if r['label']==label];b=[r for r in cf['switches'] if r['label']==label]
  if initial:
   if rf['geometry'].get('lexical_card') is not None or label in rf.get('text','') or cf['geometry'].get('lexical_card') is not None or label in cf.get('text','') or b:
    raise ValueError('Initial preference visibility/control presence differs from approved hidden source')
   continue
  if len(a)!=len(b):raise ValueError('Independent lexical field presence differs')
  if a and any(a[0][k]!=b[0][k] for k in ('selected','disabled')):raise ValueError('Independent lexical data differs')
 # Frame and every stable major content box are compared numerically before
 # strict pixels. Controls' native internal nodes may have different DOM types.
 for k in ('frame','sidebar','sidebar_footer','title','scroll','lexical_card','records_card','local_card','footer','scope'):
  if rf['geometry'].get(k)!=cf['geometry'].get(k):raise ValueError('Independent whole-client geometry differs: '+k)


def recompute(root,receipt,context):
 errors=[]
 try:
  import sys
  sys.path.insert(0,str(Path(root)/'scripts/report'))
  root=Path(root);rp=artifact(root,receipt['report']);r=load(rp);layer=receipt['layer'];refs=entries(root,layer)
  if r['execution_nonce']!=context['execution_nonce']:raise ValueError('Recompute actual dispatcher nonce differs')
  comparisons=[]
  for row in r['observations']:
   key=case_key(row['case']);reference,source_observation=reference_observation(root,refs[key],layer)
   target,target_observation,capture=raw_capture(root,row,layer,'candidate')
   if source_observation['context']['nonce']==target_observation['context']['nonce']:raise ValueError('Reference and candidate reused an execution')
   compare_semantics(source_observation,target_observation)
   if layer=='real_react':
    directory=target.parent;images=[directory/f'document-{d}-frame-{f}.png' for d in (1,2) for f in (1,2)]
   else:images=[target.parent/f['path'] for f in capture['frames']]
   for image in images:
    difference=pixels(reference,image)
    if difference['different_rgba_pixels']!=0:errors.append(f'General original {layer} {key}: {difference["different_rgba_pixels"]} RGBA pixels differ (required 0)')
    comparisons.append(difference)
  if len(r['observations'])!=27 or [v['case'] for v in r['observations']]!=list(capture_cases()):raise ValueError('Recompute fixed case closure differs')
  # A receipt cannot recycle any A/component frame into B Native: renderer
  # provenance, original filename, own process/window and source types differ.
  if layer=='packaged_native' and any('document-' in str(v) for v in [target.name]):raise ValueError('A/component snapshot substituted for OS Native frame')
 except (OSError,ValueError,KeyError,TypeError,AttributeError,ImportError) as error:errors.append('General independent recomputation failed: '+str(error))
 return errors
