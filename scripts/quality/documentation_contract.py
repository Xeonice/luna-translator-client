#!/usr/bin/env python3
"""One current reading path; exact historical bytes never satisfy current authority."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit
from gate_core import safe_path, digest
from design_authority import (CURRENT_RECORD as RECORD, current_user_approval_gaps as user_approval_gaps,
                              CURRENT_RECORD_SHA256, CURRENT_COMPOSITION_ID, CURRENT_ROUTES)

ROOT = Path(__file__).resolve().parents[2]
CONTRACT = 'docs/current-documentation.json'
LEGACY_ROOT_DESIGN = 'docs/design/first-product-v1/index.html'
SNAPSHOT = 'openspec/changes/consolidate-current-docs-and-design-gates/source-snapshot.json'
PARENT_SHA = 'c385bb1f06ae38ba83943fc4bd219768ecc71d5699f956c92b3c2eeedc0ec3bf'
PARENT_TREE = '6b7b7743d85d175e49dd8727906b411a8b565194f7d4edcaa438774810edf6ea'
ARCHIVE = 'docs/archive/documentation-r75/'
REGISTRY = {'schema_version': 2,
 'status': 'current',
 'entry': 'docs/README.md',
 'design_target': 'guided-service-reading-20261030',
 'documents': {'entry': 'docs/README.md',
               'product': 'docs/product/README.md',
               'scope': 'docs/product/first-product-scope.md',
               'model': 'docs/product/model-configuration-prd.md',
               'reading': 'docs/product/reading-window-session-prd.md',
               'design': 'docs/design/first-product-v1/README.md',
               'architecture': 'docs/architecture/first-product-engineering-baseline.md',
               'reading_technical': 'docs/architecture/reading-window-session-technical-design.md',
               'language_technical': 'docs/architecture/japanese-study-and-llm.md',
               'client': 'docs/development/desktop-client.md',
               'alignment_plan': 'docs/development/current-client-alignment-plan.md',
               'testing': 'docs/development/first-product-test-plan.md',
               'regression': 'docs/development/client-regression.md',
               'coverage': 'docs/development/current-product-test-coverage.md',
               'release': 'docs/development/release-execution-evidence.md',
               'engineering': 'docs/development/engineering-gates.md',
               'governance': 'docs/development/iteration-governance.md',
               'alignment_gate': 'docs/development/design-alignment-and-release.md',
               'page_coverage': 'docs/development/all-page-design-coverage.md',
               'history_product': 'docs/product/history-session-paging-prd.md',
               'history_plan': 'docs/development/history-session-paging-tests.md',
               'ai_product': 'docs/product/ai-reading-prd.md',
               'game_product': 'docs/product/game-library-window-prd.md',
               'guided_plan': 'docs/development/guided-service-reading-tests.md',
               'flat_plan': 'docs/development/flat-service-session-tests.md'},
 'archive_manifest': 'docs/archive/documentation-r75/manifest.json',
 'history_policy': 'historical_only; no fallback authority or inherited acceptance',
 'design_composition_id': CURRENT_COMPOSITION_ID,
 'design_authority': {'path': RECORD, 'sha256': CURRENT_RECORD_SHA256},
 'surface_entries': copy.deepcopy(CURRENT_ROUTES)}
ARCHIVED_PATHS = ('README.md', 'AGENTS.md', 'docs/product/README.md', 'docs/product/first-product-scope.md', 'docs/product/model-configuration-prd.md', 'docs/product/reading-window-session-prd.md', 'docs/design/first-product-v1/README.md', 'docs/development/current-client-alignment-plan.md', 'docs/development/first-product-test-plan.md', 'docs/development/client-regression.md', 'docs/development/release-execution-evidence.md', 'docs/development/engineering-gates.md', 'docs/development/guided-service-reading-tests.md', 'docs/development/current-product-test-coverage.md', 'docs/development/iteration-governance.md', 'docs/architecture/first-product-engineering-baseline.md', 'docs/architecture/reading-window-session-technical-design.md', 'docs/architecture/client-rebuild-architecture.md', 'docs/architecture/japanese-study-and-llm.md', 'docs/development/desktop-client.md')
PROTECTED = ('LICENSE', 'docs/product/source-manifest.json', 'docs/product/model-configuration-acceptance.csv', 'docs/product/game-library-window-acceptance.csv', 'docs/product/reading-window-session-acceptance.csv', 'docs/development/client-regression.csv', 'docs/development/first-product-test-plan.json', 'docs/development/first-product-test-executions.csv', 'docs/development/current-product-test-coverage.json', 'docs/architecture/model-service-registry-design.json', 'docs/architecture/service-configuration-presentation.json', 'docs/development/design-approval-20261019.json', 'docs/development/design-source-version-v4.json')

def _unique(pairs):
    data={}
    for key,value in pairs:
        if key in data: raise ValueError('Duplicate JSON key: '+key)
        data[key]=value
    return data

def _load(path):
    return json.loads(path.read_text(), object_pairs_hook=_unique)

def _id(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',',':'))

def _rows(text, prefix):
    return re.findall(r'^\|\s*'+prefix+r'-\d{2}\s*\|.*$', text, re.M)

def current_heading_errors(text, path):
    """Reject dated patch chapters and duplicate main chapters in current docs.

    This checks document structure, not the semantic truth of its prose. Code
    examples and ordinary dates in paragraphs remain valid; semantic agreement
    with PRD/source is still an independent review obligation.
    """
    errors, main_chapters = [], set()
    fence = None
    for line in text.splitlines():
        marker = re.match(r'^\s*(`{3,}|~{3,})', line)
        if marker:
            token = marker.group(1)
            if fence is None:
                fence = (token[0], len(token))
            elif token[0] == fence[0] and len(token) >= fence[1]:
                fence = None
            continue
        if fence is not None:
            continue
        heading = re.match(r'^(#{1,6})\s+(.+?)\s*$', line)
        if not heading:
            continue
        label = re.sub(r'\s+#+\s*$', '', heading.group(2)).strip()
        if re.search(r'(?<!\d)(?:19|20)\d{2}(?:[-/]\d{2}[-/]\d{2}|\d{4})(?!\d)', label):
            errors.append('Chronological append-only current chapter remains: ' + path)
        if len(heading.group(1)) == 2:
            key = ' '.join(label.split())
            if key in main_chapters:
                errors.append('Duplicate current main chapter: ' + path + ' -> ' + key)
            main_chapters.add(key)
    return errors

def validate_contract(root=ROOT):
    errors=[]
    try:
        root=Path(root)
        registry=_load(safe_path(root,CONTRACT))
        if _id(registry)!=_id(REGISTRY):
            errors.append('Exact unique current document roles/entry/target required; archive or second authority forbidden')
        snapshot=_load(safe_path(root,SNAPSHOT))
        before=snapshot['files']
        if (snapshot.get('parent_baseline_sha256')!=PARENT_SHA or snapshot.get('source_tree_sha256')!=PARENT_TREE
            or snapshot.get('source_entries')!=2102 or len(before)!=2102
            or hashlib.sha256(json.dumps(before,sort_keys=True).encode()).hexdigest()!=PARENT_TREE):
            errors.append('Documentation archive must bind the exact independently frozen R75 source tree')
        manifest=_load(safe_path(root,REGISTRY['archive_manifest']))
        if manifest.get('status')!='historical_only' or manifest.get('parent_baseline_id')!='FP1-20261030-R75' or manifest.get('parent_baseline_sha256')!=PARENT_SHA:
            errors.append('Archive must retain historical-only R75 parent identity')
        entries=manifest.get('entries',[])
        if not isinstance(entries,list) or len(entries)!=len(ARCHIVED_PATHS) or {e.get('original') for e in entries}!=set(ARCHIVED_PATHS):
            errors.append('Exact complete historical snapshot inventory required')
        for e in entries:
            original=e['original']; path=ARCHIVE+original
            if (e.get('snapshot')!=path or e.get('sha256')!=before.get(original)
                or type(e.get('bytes')) is not int or e['bytes']<=0
                or safe_path(root,path).stat().st_size!=e['bytes'] or digest(safe_path(root,path))!=e['sha256']):
                errors.append('Historical snapshot byte/hash identity changed: '+original)
        for p in PROTECTED:
            if digest(safe_path(root,p))!=before[p]:
                errors.append('Frozen plan/original/approved asset changed during consolidation: '+p)
        for p in REGISTRY['documents'].values():
            text=safe_path(root,p).read_text()
            if not text.strip(): errors.append('Empty current document: '+p)
            errors.extend(current_heading_errors(text, p))
            # Local link existence (archive snapshots retain original relative text).
            for raw in re.findall(r'\[[^\]]*\]\(([^)]+)\)',text):
                u=urlsplit(raw.strip('<>'))
                if u.scheme or u.netloc or not u.path: continue
                target=(root/p).parent/unquote(u.path)
                if not target.resolve().is_relative_to(root.resolve()) or not target.exists():
                    errors.append('Broken/outside current document link: '+p+' -> '+raw)
                current_entries = {(root / route['entry']).resolve()
                                   for route in REGISTRY['surface_entries'].values()}
                if (target.resolve() == (root / LEGACY_ROOT_DESIGN).resolve()
                        and target.resolve() not in current_entries):
                    errors.append('Retired root prototype cannot be a current design entry: '+p+' -> '+raw)
        for p,prefix in [('docs/product/first-product-scope.md','FP'),('docs/product/model-configuration-prd.md','OT')]:
            current=_rows(safe_path(root,p).read_text(),prefix)
            old=_rows(safe_path(root,ARCHIVE+p).read_text(),prefix)
            if not current or current!=old: errors.append('Consolidation changed stable requirement rows: '+prefix)
        p='docs/product/model-configuration-prd.md'
        old=re.findall(r'^### AI-CFG-\d{2} .*$',safe_path(root,ARCHIVE+p).read_text(),re.M)
        if re.findall(r'^### AI-CFG-\d{2} .*$',safe_path(root,p).read_text(),re.M)!=old or len(old)!=16:
            errors.append('All 16 stable AI-CFG requirement headings must remain unchanged')
        hub=safe_path(root,'docs/README.md').read_text()
        for p in ('README.md','AGENTS.md'):
            if 'docs/README.md' not in safe_path(root,p).read_text(): errors.append('Root/agent must route to current hub: '+p)
        errors += user_approval_gaps(root)
        approval_links = []
        for raw in re.findall(r'\[[^\]]*\]\(([^)]+)\)', hub):
            u = urlsplit(raw.strip('<>'))
            if not u.scheme and not u.netloc and u.path:
                approval_links.append(((root / 'docs/README.md').parent / unquote(u.path)).resolve())
        if ('approved_by_user' not in hub or REGISTRY['design_target'] not in hub
                or 'blocked' not in hub or (root / RECORD).resolve() not in approval_links):
            errors.append('Current hub must state effective approved_by_user with the exact decision record link and blocked product release')
    except (OSError,ValueError,TypeError,KeyError,AttributeError) as e:
        errors.append('Current documentation contract invalid: '+str(e))
    return errors

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true',required=True);parser.add_argument('--json',action='store_true');parser.add_argument('--root',type=Path,default=ROOT);args=parser.parse_args()
    errors=validate_contract(args.root);report={'schema_version':1,'status':'failed' if errors else 'passed','errors':errors,'archived_documents':len(ARCHIVED_PATHS),'not_product_execution':True,'product_tests_executed':False}
    print(json.dumps(report,ensure_ascii=False,indent=2) if args.json else report['status'])
    return bool(errors)
if __name__=='__main__': raise SystemExit(main())
