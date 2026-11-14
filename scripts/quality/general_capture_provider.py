#!/usr/bin/env python3
"""Explicit migration of the General V3 pixel observer, not its obligations.

The original Chromium images and byte/8-canvas/saved-game/zero assertions stay in
G01/G17. Strict relocation pixels now use two fresh same-target-engine WK pages.
Nothing here grants source approval, reference review or a product layer pass.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts/quality'))
sys.path.insert(0,str(ROOT/'scripts/report'))
from current_general_settings_authority import approval_inputs

IMPLEMENTATION_INPUTS=(
 'scripts/quality/current_general_settings_authority.py','scripts/quality/gate_core.py',
 'scripts/quality/current_general_visual_authority.py','scripts/quality/general_capture_provider.py',
 'scripts/report/general_visual_webkit.py','scripts/report/general_visual_webkit_host.swift',
 'apps/desktop/tests/visual/general-source-capture-provider.mjs',
)

def inputs():return sorted(set(approval_inputs(ROOT))|set(IMPLEMENTATION_INPUTS))

def migration_contract():
 return {'kind':'general-v3-capture-observer-migration/v1','source':'user-approved-v3-unchanged-content',
         'original_observer':'locked-chromium-cross-document','original_observer_qualified':False,
         'retained_obligations':['two_dependency_urls_only','remaining_files_verbatim','eight_complete_canvas_combinations','saved_game_override','zero_rgba_pixels','blank_workspace_reproduction'],
         'replacement_observer':'public-WKWebView-same-target-engine-two-fresh-nonpersistent-documents-two-fixed-frames',
         'chromium_originals':'retained_unqualified_diagnostics_not_product_failure_or_pass',
         'threshold':0,'mask':False,'crop':False,'resample':False,'retry_selection':False,
         'packaged_native':False,'product_accepted':False,'reference_approved':False,
         'requires_independent_code_and_original_png_review':True}

def main():
 p=argparse.ArgumentParser();p.add_argument('--inputs',action='store_true');p.add_argument('--contract',action='store_true');p.add_argument('--canvas');p.add_argument('--saved-game',action='store_true');p.add_argument('--output');a=p.parse_args()
 if a.inputs or a.contract:
  if a.canvas or a.saved_game or a.output or (a.inputs and a.contract):p.error('Mode arguments are exclusive')
  print(json.dumps(inputs() if a.inputs else migration_contract(),ensure_ascii=False));return 0
 if not a.canvas or not a.output:p.error('A fixed canvas and fresh private output are required')
 from general_visual_webkit import safe_output,execute
 out=safe_output(a.output)
 result=execute(out,a.canvas,'source-saved-game' if a.saved_game else 'source-qualification')
 print(json.dumps({'status':result['status'],'result':str((out/'result.json').relative_to(ROOT)),'output':result['output'],'contract':migration_contract()}))
 return 0 if result['status']=='passed' else 1

if __name__=='__main__':
 try:sys.exit(main())
 except (ValueError,KeyError,OSError) as e:print(json.dumps({'status':'rejected','reason':str(e)}),file=sys.stderr);sys.exit(2)
