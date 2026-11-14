"""Finite General controlled visual adapters; root/product/global gates stay strict."""
from __future__ import annotations
from datetime import datetime,timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
from gate_core import safe_path,digest
from current_general_visual_authority import STATE_IDS as IDS,capture_cases,case_key,authority,SCOPE
from current_general_reference import load
REACT_CHECK='product-general-visual-react-tests'
NATIVE_CHECK='product-general-visual-native-tests'
REACT_COMMAND=('node','scripts/report/verify_general_visual_react.mjs')
NATIVE_COMMAND=('python3','scripts/report/verify_general_native_visual.py')
STATE_IDS=frozenset(IDS)

def now():return datetime.now(timezone.utc).isoformat()
def write(path,value):
 with Path(path).open('x') as f:json.dump(value,f,ensure_ascii=False,indent=2);f.write('\n')
def ref(root,path):return {'path':path.relative_to(root).as_posix(),'sha256':digest(path)}
def actual(command,root,path,timeout):
 started=now()
 child=subprocess.Popen(command,cwd=root,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 try:
  stdout,stderr=child.communicate(timeout=timeout);value={'command':command,'cwd':str(root),'pid':child.pid,'parent_pid':os.getpid(),'started_at':started,'finished_at':now(),'exit_code':child.returncode,'stdout':stdout,'stderr':stderr,'timed_out':False}
 except subprocess.TimeoutExpired:
  child.terminate()
  try: stdout,stderr=child.communicate(timeout=8)
  except subprocess.TimeoutExpired: stdout,stderr='', 'Owned controller did not exit after SIGTERM; no force-kill used'
  value={'command':command,'cwd':str(root),'pid':child.pid,'parent_pid':os.getpid(),'started_at':started,'finished_at':now(),'exit_code':None,'stdout':stdout,'stderr':stderr,'timed_out':True}
 write(path,value)
 return value


def execute(root,context):
 from package_identity import inspect_package
 from current_general_visual_inspect import inspect
 from current_general_visual_recompute import recompute
 root=Path(root);layer=context['layer'];identity=REACT_CHECK if layer=='real_react' else NATIVE_CHECK;command=REACT_COMMAND if layer=='real_react' else NATIVE_COMMAND
 if layer not in ('real_react','packaged_native') or context['adapter_id']!=identity or context['command']!=list(command) or context['state_ids']!=sorted(STATE_IDS):raise ValueError('Finite General adapter invocation differs')
 output=safe_path(root,context['output']);request=output/'request.json'
 if load(request)!=context:raise ValueError('Actual General dispatcher request differs')
 package=safe_path(root,context['artifact_path']);attestation=inspect_package(root,package,context['source_tree_sha256'],output/'package-inspection')
 started=now();execution_path=output/'actual-command.json';report_path=output/'capture/report.json'
 if layer=='real_react':
  node=shutil.which('node')
  if not node:raise ValueError('Node unavailable')
  invoked=[str(Path(node).resolve()),str(safe_path(root,command[1])),'--request',str(request),'--output',str(output)]
  execution=actual(invoked,root,execution_path,600)
 else:
  app=package.parent/'Luna.app'
  if not app.is_dir() or app.is_symlink():raise ValueError('Same inspected archive sibling signed Luna.app required')
  capture=output/'capture';capture.mkdir();commands=[];rows=[]
  for row in capture_cases():
   directory=capture/case_key(row)
   invoked=[str(Path(shutil.which('python3')).resolve()),str(safe_path(root,NATIVE_COMMAND[1])),'--app',str(app),'--expected-source',context['source_tree_sha256'],'--side','candidate','--state',row['state_id'],'--canvas',row['canvas_id'],'--subcase',row['subcase'],'--output',str(directory)]
   child=actual(invoked,root,capture/(case_key(row)+'.command.json'),140);commands.append(child)
   rows.append({'case':row,'side':'candidate','raw_directory':directory.relative_to(root).as_posix()})
   if child['exit_code']!=0:break
  finished=now();execution={'command':list(NATIVE_COMMAND),'cwd':str(root),'started_at':started,'finished_at':finished,'exit_code':0 if len(rows)==27 and all(c['exit_code']==0 for c in commands) else 1,'actual_commands':commands,'package_inspection':attestation}
  write(execution_path,execution)
  report={'schema_version':1,'kind':'general-packaged-native-execution/v1','layer':layer,'scope':SCOPE,'source_authority':authority(root),'context':context,'execution_nonce':context['execution_nonce'],'source_tree_sha256':context['source_tree_sha256'],'artifact_sha256':context['artifact_sha256'],'observations':rows,'started_at':started,'finished_at':finished,'collection_complete':len(rows)==27,'native_verified':True,'packaged_native_tests_executed':True,'game_tests_executed':False,'os_runtime_verified':False,'storage_runtime_verified':False}
  write(report_path,report)
 if not report_path.is_file():raise ValueError('Actual General producer failed before raw report; command retained')
 report=load(report_path)
 receipt={k:context[k] for k in ('schema_version','adapter_id','target_id','composition_id','layer','design_closure_sha256','source_tree_sha256','artifact_sha256','command','execution_nonce','state_ids')}
 receipt.update(status='failed',errors=[],producer_kind=layer,prototype_substitution=False,native_verified=layer=='packaged_native',started_at=report['started_at'],finished_at=report['finished_at'],assertion_count=len(report.get('observations',[]))*8,skipped_assertions=0,report=ref(root,report_path),execution=ref(root,execution_path),controlled_visual_only=True,game_tests_executed=False,os_runtime_verified=False,storage_runtime_verified=False)
 check_context={k:receipt[k] for k in ('target_id','composition_id','design_closure_sha256','layer','state_ids','artifact_sha256','execution_nonce')}
 findings={'inspection':inspect(root,receipt,check_context),'independent_recomputation':recompute(root,receipt,check_context)}
 write(output/'raw-validation.json',findings)
 receipt['errors']=findings['inspection']+findings['independent_recomputation']+([] if execution['exit_code']==0 else ['Actual General command failed'])
 receipt['status']='passed' if not receipt['errors'] else 'failed';write(output/'producer-receipt.json',receipt)
 if load(request)!=context:raise ValueError('Producer rewrote General dispatcher request')
 return receipt
