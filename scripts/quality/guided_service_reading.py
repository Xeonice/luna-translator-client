#!/usr/bin/env python3
"""Verify the user-facing configuration/reading candidate, without running services."""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib,json,re,subprocess,sys,zlib,struct,itertools
from urllib.parse import urlsplit
from pathlib import Path
import flat_service_session as parent
ROOT=Path(__file__).resolve().parents[2]
CATALOG=parent.CATALOG
PRESENTATION='docs/architecture/service-configuration-presentation.json'
DESIGN_ROOT=parent.DESIGN_ROOT
CANDIDATE_ID='guided-service-reading-20261030'
CANDIDATE=DESIGN_ROOT+'/candidates/'+CANDIDATE_ID
DESIGN=CANDIDATE+'/index.html'
MANIFEST=CANDIDATE+'/manifest.json'
CHECKER='scripts/quality/guided_service_reading.py'
TESTS='scripts/quality/test_guided_service_reading.py'
RUNNER='scripts/report/verify_guided_service_reading.cjs'
PLAN='docs/development/guided-service-reading-tests.md'
GD_IDS={f'GD-{n:02d}' for n in range(1,13)}
_safe=parent._safe
_load_json=parent._load_json
_output_path=parent._output_path
_text=parent._text
_strings=parent._strings
_positive=parent._positive
SHA256=parent.SHA256


CLASSES=['required','conditional','default','advanced','readonly','notapplicable']
FIELD_TYPES=['text','url','secret','select','path','number','boolean','textarea','status','action']

def valid_url(value):
    if not isinstance(value,str):return False
    try:
        url=urlsplit(value)
        return url.scheme in {'http','https'} and bool(url.hostname) and url.username is None and url.password is None
    except ValueError:return False


def valid_condition(condition,ids):
    if not isinstance(condition,dict) or set(condition)-{'all','any'} or not isinstance(condition.get('all'),list):return False
    if 'any'in condition and not condition['any']:return False
    for key in ['all','any']:
        clauses=condition.get(key,[])
        if not isinstance(clauses,list):return False
        for c in clauses:
            if not isinstance(c,dict) or set(c)!={'field','op','value'} or c.get('field')not in ids:return False
            v=c['value']
            if c.get('op')=='eq':
                if not isinstance(v,(str,bool,int,float)) or isinstance(v,float) and (v!=v or abs(v)==float('inf')):return False
            elif c.get('op')=='in':
                if not isinstance(v,list) or not v or any(not isinstance(x,(str,bool,int,float)) for x in v):return False
            else:return False
    return True


def matches(condition,values):
    def clause(c):return values.get(c['field'])==c['value'] if c['op']=='eq' else values.get(c['field'])in c['value']
    return all(clause(c) for c in condition['all']) and (not condition.get('any') or any(clause(c) for c in condition['any']))


def endpoint_materials(service):
    fields={f['id']:f for f in service['fields']};endpoint=fields.get('endpoint',{})
    by=endpoint.get('default_by');controllers=set()
    if by is not None:
        if not isinstance(by,dict) or set(by)!={'field','values'} or by['field']not in fields or not isinstance(by['values'],dict):return False
        controllers.add(by['field'])
    blockers=service.get('preset_material_blockers',[])
    for b in blockers:
        if isinstance(b,dict) and valid_condition(b.get('condition'),fields):controllers.update(c['field'] for k in ['all','any'] for c in b['condition'].get(k,[]))
    controllers.discard('connection_mode');names=sorted(controllers);choices=[]
    for name in names:
        f=fields[name];values=[x.get('value') for x in f.get('choices',[]) if isinstance(x,dict)]
        if not values:values=[f.get('default_value')]
        choices.append(values)
    if any(not v for v in choices) or sum(len(v) for v in choices)>100:return False
    for option in itertools.product(*choices):
        values={k:f.get('default_value') for k,f in fields.items()};values.update(zip(names,option));values['connection_mode']='preset'
        url=endpoint.get('default_value') if by is None else by['values'].get(values.get(by['field']))
        if valid_url(url):continue
        if not any(isinstance(b,dict) and valid_condition(b.get('condition'),fields) and matches(b['condition'],values) for b in blockers):return False
    return True


def validate_catalog(registry,presentation):
    errors=parent.validate_catalog(registry)
    if not isinstance(registry,dict) or not isinstance(presentation,dict):return errors+['Guided registry/presentation objects required']
    d=registry.get('guided_configuration_design',{})
    if not isinstance(d,dict):return errors+['guided_configuration_design object required']
    for k,v in {'change_id':'design-guided-service-and-reading','phase':'design_documentation_only','candidate_design':DESIGN+'?scenario=reading','candidate_configuration':DESIGN+'?scenario=settings','candidate_design_status':'pending_review','presentation_catalog':PRESENTATION}.items():
        if d.get(k)!=v:errors.append('Guided contract mismatch: '+k)
    boundary=d.get('scope_boundaries',{})
    for k,v in {'http_protocols':13,'http_presets':21,'offline_engines':4,'all_prior_registry_keys_unchanged':True,'original_plans_and_statuses_unchanged':True,'rust_authority_and_narrow_tauri_unchanged':True,'production_source_or_package_changed':False,'database_or_contract_changed':False,'approved_png_tokens_and_prior_candidates_changed':False,'product_acceptance':'not_run'}.items():
        if not isinstance(boundary,dict) or type(boundary.get(k)) is not type(v) or boundary.get(k)!=v:errors.append('Guided boundary mismatch: '+k)
    if presentation.get('counts')!={'http_presets':21,'http_protocols':13,'offline_engines':4,'services':25}:errors.append('Exact guided scope counts required')
    reading=d.get('reading',{})
    for k,v in {'placement':'after_primary_reading','default_collapsed':True,'secondary_regions':['body_history','all_extraction_events','hook_diagnostics'],'full_search_export_and_native_auditing_preserved':True}.items():
        if not isinstance(reading,dict) or type(reading.get(k)) is not type(v) or reading.get(k)!=v:errors.append('Guided reading contract mismatch: '+k)
    for k,v in {'status':'pending_review','phase':'design_documentation_only','product_acceptance':'not_run','source_registry':CATALOG}.items():
        if presentation.get(k)!=v:errors.append('Presentation identity mismatch: '+k)
    rows=registry.get('guided_acceptance',[])
    if not isinstance(rows,list) or any(not isinstance(x,dict) for x in rows):return errors+['Guided acceptance rows required']
    ids=[x.get('id') for x in rows]
    if not _strings(ids) or set(ids)!=GD_IDS:errors.append('GD-01 through GD-12 must occur exactly once')
    for row in rows:
        for key,pattern in [('source_configuration_requirement_ids',r'AI-CFG-(0[1-9]|1[0-6])'),('source_reading_requirement_ids',r'RW-(0[1-9]|[12][0-9]|3[0-4])')]:
            values=row.get(key)
            if not isinstance(values,list) or (values and (not _strings(values) or any(not re.fullmatch(pattern,x) for x in values))):errors.append('Invalid guided requirement mapping: '+key)
        if row.get('status')!='not_run':errors.append('Guided product status must remain not_run')
        refs=row.get('source_requirement_ids');layers=row.get('verification_layers')
        if not _strings(refs) or any(not re.fullmatch(r'FP-(0[1-9]|1[0-2])',x) for x in refs):errors.append('Guided FP mappings required')
        if not _strings(layers) or 'design' not in layers or len(layers)<2:errors.append('Guided design and future real layers required')
    if presentation.get('classifications')!=CLASSES or presentation.get('types')!=FIELD_TYPES:errors.append('Field DSL enums are fixed, not self-authorized')
    services=presentation.get('services');brands=presentation.get('brands');sources=presentation.get('sources')
    if not isinstance(services,list) or any(not isinstance(x,dict) for x in services) or not isinstance(brands,list) or any(not isinstance(x,dict) for x in brands) or not isinstance(sources,dict):return errors+['Guided services/brands/source descriptors required']
    for source in sources.values():
        if not isinstance(source,dict) or source.get('kind')not in {'local_contract','official','official_search_index'} or not _text(source.get('ref')):errors.append('Exact source kind/ref required');continue
        ref=source['ref']
        if source['kind']=='local_contract':
            if Path(ref.split('#')[0]).is_absolute() or '..'in Path(ref.split('#')[0]).parts:errors.append('Local source path must stay inside workspace')
        elif not valid_url(ref):errors.append('Official source must have a valid HTTP URL')
    protocol_map={x['id']:x for x in registry['protocols']}
    known={x['id']:x for x in registry['presets']+registry['offline_engines']}
    ids=[x.get('id') for x in services]
    if not _strings(ids) or set(ids)!=set(known):errors.append('Presentation must cover the exact 21 presets and 4 offline engines')
    brandids=[b.get('id') for b in brands]
    if not _strings(brandids):errors.append('Unique named brand identities required')
    assignments=[x for b in brands for x in b.get('service_ids',[])]
    if not _strings(assignments) or set(assignments)!=set(known):errors.append('Brands must assign every exact interface once')
    for brand in brands:
        if brand.get('default_service_id')not in brand.get('service_ids',[]):errors.append('Brand default must be its own explicit interface')
    for service in services:
        identity=service.get('id');original=known.get(identity)
        if original is None:continue
        is_http='protocol_id' in original
        if service.get('kind')!=('http' if is_http else 'offline') or service.get('allowed_purposes')!=original['allowed_purposes']:errors.append(f'{identity}: kind/purpose changed')
        for dest,src in ([('protocol_id','protocol_id'),('profile_id','compatibility_profile_id'),('adapter_contract_revision','adapter_contract_revision'),('profile_revision','profile_revision'),('preset_revision','preset_revision')] if is_http else [('execution_backend','execution_backend'),('engine_contract_revision','engine_contract_revision')]):
            if type(service.get(dest)) is not type(original[src]) or service.get(dest)!=original[src]:errors.append(f'{identity}: exact {dest} identity required')
        if not any(b.get('id')==service.get('brand_id') and identity in b.get('service_ids',[]) for b in brands):errors.append(f'{identity}: brand assignment mismatch')
        derived=protocol_map.get(original.get('protocol_id'),original)
        if service.get('model_identity')!=derived.get('model_identity'):errors.append(f'{identity}: exact model identity required')
        if is_http and service.get('auth_kind')!=derived.get('auth_kind'):errors.append(f'{identity}: exact auth kind required')
        if service.get('brand_id')not in brandids or service.get('execution_status')!='not_run':errors.append(f'{identity}: brand/status invalid')
        fields=service.get('fields')
        if not isinstance(fields,list) or any(not isinstance(f,dict) for f in fields):errors.append(f'{identity}: field list required');continue
        fieldids=[f.get('id') for f in fields]
        if not _strings(fieldids):errors.append(f'{identity}: unique field IDs required');continue
        for blocker in service.get('preset_material_blockers',[]):
            if not isinstance(blocker,dict) or not _text(blocker.get('code')) or not _text(blocker.get('reason')) or not _strings(blocker.get('source_refs')) or any(x not in sources for x in blocker.get('source_refs',[])):errors.append(f'{identity}: preset blocker identity/reason/source required');continue
            condition=blocker.get('condition')
            if not valid_condition(condition,fieldids):errors.append(f'{identity}: invalid preset material condition')
        for field in fields:
            fid=field['id'];cl=field.get('classification');condition=field.get('condition')
            if cl not in presentation.get('classifications',[]) or field.get('type')not in presentation.get('types',[]) or not _text(field.get('label')):errors.append(f'{identity}.{fid}: typed user field required')
            refs=field.get('source_refs')
            if not _strings(refs) or any(x not in sources for x in refs):errors.append(f'{identity}.{fid}: exact sources required')
            if not valid_condition(condition,fieldids):errors.append(f'{identity}.{fid}: explicit conditions required');continue
            for group in ['all','any']:
                clauses=condition.get(group,[])
                if not isinstance(clauses,list):errors.append(f'{identity}.{fid}: condition list required');continue
                for clause in clauses:
                    if not isinstance(clause,dict) or clause.get('field')not in fieldids or clause.get('op')not in ['eq','in'] or 'value'not in clause:errors.append(f'{identity}.{fid}: invalid condition dependency')
            if field.get('type')=='secret' and (field.get('masked')is not True or 'default_value'in field):errors.append(f'{identity}.{fid}: secret must be masked and cannot have a default')
            if fid in {'signature','salt','curtime','credential_ref','schema_version','protocol_id','contract_revision'} and cl in {'required','conditional'}:errors.append(f'{identity}.{fid}: internal field cannot be user-required')
            if is_http and fid=='endpoint' and (cl!='conditional' or {'field':'connection_mode','op':'eq','value':'custom'}not in condition['all']):errors.append(f'{identity}: built-in URL may show only on explicit custom intent')
            if fid=='model' and ('default_value'in field or 'default_by'in field):errors.append(f'{identity}: example model cannot be a selected account default')
        if is_http and not endpoint_materials(service):errors.append(f'{identity}: every preset endpoint branch must have an evidenced HTTP address or explicit material blocker')
        if not is_http:
            if service.get('unknown_required_condition_blocks')is not True:errors.append(f'{identity}: local unknown gate required')
            if any(f['id'] in {'endpoint','api_key','model'} and f.get('classification')in {'required','conditional'} for f in fields):errors.append(f'{identity}: offline cannot require HTTP/LLM fields')
    internal=presentation.get('internal_fields')
    if not isinstance(internal,list) or not internal or any(not isinstance(x,dict) or x.get('classification')!='notapplicable' for x in internal):errors.append('Internal fields must never become user-editable')
    return errors


def manifest_sources(root):
    m=_load_json(_safe(root,MANIFEST));paths=set()
    for k,v in {'schema_version':1,'candidate_id':CANDIDATE_ID,'entry':DESIGN,'status':'pending_review','runtime_base_href':'../../','parent_candidate_id':parent.CANDIDATE_ID}.items():
        if type(m.get(k)) is not type(v) or m.get(k)!=v:raise ValueError('Guided manifest invalid '+k)
    owned=m.get('owned_sources');shared=m.get('shared_dependencies');prior=m.get('parent_sources_unchanged')
    if not isinstance(owned,dict) or not isinstance(shared,dict) or not shared or not isinstance(prior,dict):raise ValueError('Guided dependency groups required')
    if set(owned)!={p.name for p in (root/CANDIDATE).iterdir() if p.is_file() and p.name!='manifest.json'}:raise ValueError('Guided manifest must bind all owned files')
    if set(prior)!={p.name for p in (root/parent.CANDIDATE).iterdir() if p.is_file()}:raise ValueError('Guided parent source closure required')
    for group,prefix in [(owned,CANDIDATE),(shared,DESIGN_ROOT),(prior,parent.CANDIDATE)]:
        for rel,digest in group.items():
            if not _text(rel) or not isinstance(digest,str) or not SHA256.fullmatch(digest):raise ValueError('Invalid guided source path/hash')
            if group is not shared and len(Path(rel).parts)!=1:raise ValueError('Guided owned/parent path must be top-level')
            p=prefix+'/'+rel
            if hashlib.sha256(_safe(root,p).read_bytes()).hexdigest()!=digest:raise ValueError('Guided source identity mismatch: '+p)
            paths.add(p)
    assets=parent.parent._references(_safe(root,DESIGN).read_text())
    if not {DESIGN_ROOT+'/'+a for a in assets}.issubset(paths):raise ValueError('Guided manifest lacks loaded dependencies')
    if not {'candidates/'+CANDIDATE_ID+'/'+x for x in ['guided-catalog.js','guided-config.js','flat-layout.js','flat-layout.css']}.issubset(assets):raise ValueError('Guided presentation adapters not loaded')
    return paths


def generated_catalog(root):
    text=_safe(root,CANDIDATE+'/guided-catalog.js').read_text();decoder=json.JSONDecoder()
    def assigned(name):
        marker='window.'+name+' = ';assert text.count(marker)==1,'Exact generated assignment required: '+name
        return decoder.raw_decode(text.split(marker,1)[1].lstrip())[0]
    catalog=_load_json(_safe(root,PRESENTATION));registry=_load_json(_safe(root,CATALOG))
    if assigned('LunaGuidedCatalog')!=catalog:raise ValueError('Generated UI field catalog differs from reviewed JSON')
    if assigned('LunaGuidedCatalogSource')!={'path':PRESENTATION,'sha256':hashlib.sha256(_safe(root,PRESENTATION).read_bytes()).hexdigest()}:raise ValueError('Generated catalog source identity mismatch')
    if assigned('LunaGuidedProtocolMetadata')!=registry['protocols']:raise ValueError('Generated protocol metadata differs from exact registry')


def validate_plan(body):
    rows=re.findall(r'^\|\s*(GD-\d{2})\s*\|.*$',body,re.M);errors=[]
    if len(rows)!=12 or set(rows)!=GD_IDS:errors.append('Plan requires GD01 through12 exactly once')
    if any(not re.search(r'[;；]\s*not_run\s*\|\s*$',row) for row in re.findall(r'^\|\s*GD-\d{2}\s*\|.*$',body,re.M)):errors.append('Guided product plan states must remain not_run')
    return errors


def check(root=ROOT):
    root=Path(root).resolve();errors=[]
    try:
        errors.extend(parent.check(root));registry=_load_json(_safe(root,CATALOG));p=_load_json(_safe(root,PRESENTATION));errors.extend(validate_catalog(registry,p));manifest_sources(root);generated_catalog(root);errors.extend(validate_plan(_safe(root,PLAN).read_text()))
        for ref in registry['guided_configuration_design']['source_documents']:
            path,_,anchor=ref.partition('#');body=_safe(root,path).read_text()
            if anchor and f'id="{anchor}"'not in body:errors.append('Guided source anchor missing: '+ref)
    except (OSError,ValueError,TypeError,KeyError,UnicodeError,AttributeError,AssertionError) as error:errors.append('Guided design cannot be checked: '+str(error))
    return errors


def _source_hashes(root):
    paths=manifest_sources(root)|{CATALOG,PRESENTATION,PLAN,CHECKER,TESTS,RUNNER,MANIFEST,'scripts/quality/flat_service_session.py','scripts/quality/translation_service_selection.py','scripts/quality/offline_translation_design.py','openspec/technology-lock.json','docs/development/iteration-baseline.json'}
    for ref in _load_json(_safe(root,CATALOG))['guided_configuration_design']['source_documents']:paths.add(ref.split('#')[0])
    return {p:hashlib.sha256(_safe(root,p).read_bytes()).hexdigest() for p in sorted(paths)}


def png_valid(data):
    try:
        if data[:8]!=b'\x89PNG\r\n\x1a\n':return False
        offset=8;chunks=[];compressed=[];header=None;ended=False
        while offset<len(data):
            if offset+12>len(data):return False
            n=int.from_bytes(data[offset:offset+4],'big');kind=data[offset+4:offset+8];end=offset+12+n
            if end>len(data):return False
            raw=data[offset+8:offset+8+n];crc=int.from_bytes(data[offset+8+n:end],'big')
            if zlib.crc32(kind+raw)&0xffffffff!=crc:return False
            chunks.append(kind)
            if kind==b'IHDR':
                if header is not None or n!=13 or len(chunks)!=1:return False
                header=struct.unpack('>IIBBBBB',raw)
            elif kind==b'IDAT':compressed.append(raw)
            elif kind==b'IEND':
                if n!=0 or end!=len(data):return False
                ended=True
            offset=end
        if not header or not ended or not compressed:return False
        width,height,depth,color,compression,filter_method,interlace=header
        if not width or not height or depth not in {1,2,4,8,16} or color not in {0,2,3,4,6} or compression or filter_method or interlace:return False
        row=(width*{0:1,2:3,3:1,4:2,6:4}[color]*depth+7)//8
        expected=(row+1)*height
        if expected>512*1024*1024:return False
        inflater=zlib.decompressobj();raw=inflater.decompress(b''.join(compressed),expected+1)
        return len(raw)==expected and inflater.eof and not inflater.unused_data and all(raw[n*(row+1)]<=4 for n in range(height))
    except (ValueError,struct.error,zlib.error,OverflowError):return False


def _adapter_report(destination: Path, root: Path | None = None) -> tuple[dict, list[str]]:
    """Independently verify named scenarios and PNG identities, never exit alone."""
    path = _safe(destination, "report.json")
    report = _load_json(path)
    if not isinstance(report, dict):
        return {}, ["Node/Playwright report must be a JSON object"]
    errors: list[str] = []
    if (type(report.get("schema_version")) is not int or report.get("schema_version") != 1
            or report.get("kind") != "guided_service_reading_design_verification"
            or report.get("layer") != "design" or report.get("status") != "passed"
            or report.get("product_tests_executed") is not False):
        errors.append("Node/Playwright report must identify a passed selection design-only execution")
    if not _positive(report.get("assertions")) or report["assertions"] < len(GD_IDS):
        errors.append("Node/Playwright report must contain actual assertions for all twelve scenarios")
    checks = report.get("checks")
    if not isinstance(checks, list) or not checks or any(not isinstance(row, dict) for row in checks):
        errors.append("Node/Playwright report requires unique passed GD scenario checks")
        checks = []
    ids = [row.get("id") for row in checks]
    if (not _strings(ids) or set(ids) != GD_IDS
            or any(row.get("status") != "passed" for row in checks)):
        errors.append("Node/Playwright report must pass every GD-01 through GD-12 exactly once")
    if any(not _positive(row.get("assertions")) for row in checks):
        errors.append("Every GD scenario must have positive actual assertions")
    if report.get("page_errors") != [] or report.get("network_requests") != [] or report.get("console_errors") != []:
        errors.append("Browser errors or external network requests reject design acceptance")
    regressions=report.get('regression_checks')
    required_regressions={f'GUI{n:02d}' for n in range(1,9)}
    if not isinstance(regressions,list) or any(not isinstance(x,dict) for x in regressions):errors.append('GUI regression evidence required')
    else:
        names=[x.get('id') for x in regressions]
        if not _strings(names) or set(names)!=required_regressions or any(x.get('status')!='passed' or not _positive(x.get('assertions')) for x in regressions):errors.append('All eight independently discovered GUI regressions require passed assertions')
    before,after=report.get('source_hashes'),report.get('source_hashes_after')
    if not isinstance(before,dict) or not before or before!=after or any(not isinstance(k,str) or not isinstance(v,str) or not SHA256.fullmatch(v) for k,v in (before or {}).items()):errors.append('Node source identity must be nonempty and stable')
    if root is not None:
        manifest=_load_json(_safe(root,MANIFEST));paths={MANIFEST,RUNNER,CATALOG,PRESENTATION}
        for key,prefix in [('owned_sources',CANDIDATE),('shared_dependencies',DESIGN_ROOT)]:paths.update(prefix+'/'+x for x in manifest[key])
        expected={x:hashlib.sha256(_safe(root,x).read_bytes()).hexdigest() for x in paths}
        if before!=expected or after!=expected:errors.append('Node source closure differs from independently hashed current inputs')
    screenshots = report.get("screenshots")
    if not isinstance(screenshots, list) or not screenshots or any(not isinstance(row, dict) for row in screenshots):
        errors.append("Node/Playwright report requires actual screenshot evidence")
        screenshots = []
    if not _strings([row.get("path") for row in screenshots]):
        errors.append("Node/Playwright screenshot paths must be unique strings")
    for row in screenshots:
        try:
            relative = row.get("path")
            if not _text(relative) or Path(relative).suffix.lower() != ".png":
                raise ValueError("Screenshot evidence must name a local PNG")
            data = _safe(destination, relative).read_bytes()
            if not png_valid(data):raise ValueError('PNG evidence is truncated, corrupt or unsupported: '+relative)
            if (not _positive(row.get("bytes")) or row.get("bytes") != len(data)
                    or row.get("sha256") != hashlib.sha256(data).hexdigest()):
                raise ValueError(f"Screenshot byte/hash identity mismatch: {relative}")
            if (len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR"
                    or not _positive(row.get("width")) or not _positive(row.get("height"))
                    or row["width"] != int.from_bytes(data[16:20], "big")
                    or row["height"] != int.from_bytes(data[20:24], "big")):
                raise ValueError(f"Screenshot PNG dimensions mismatch: {relative}")
        except (OSError, ValueError, TypeError) as error:
            errors.append(str(error))
    return {"path": "report.json", "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "assertions": report.get("assertions"), "checks": len(checks)}, errors


def execute(root: Path, output: Path) -> dict:
    """Run Node/Playwright and retain a new design-only receipt even on failure."""
    destination = _output_path(Path(root), Path(output))
    root = Path(root).resolve()
    errors = check(root)
    destination.mkdir(parents=True, exist_ok=False)
    receipt = {"schema_version": 1, "kind": "guided_service_reading_design_execution",
               "layer": "design_only", "product_acceptance": "not_run", "candidate_design_status": "pending_review",
               "status": "failed", "started_at": datetime.now(timezone.utc).isoformat(),
               "root": str(root), "output": str(destination), "static_errors": errors,
               "command": None, "exit_code": None, "source_hashes_before": {}, "source_hashes_after": {}, "errors": []}
    stdout_path, stderr_path = destination / "stdout.log", destination / "stderr.log"
    stdout_path.touch()
    stderr_path.touch()
    try:
        if not errors:
            before = _source_hashes(root)
            receipt["source_hashes_before"] = before
            command = ["node", str(_safe(root, RUNNER)), str(root), str(destination)]
            receipt["command"] = command
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                try:
                    completed = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr, check=False, timeout=300)
                    receipt["exit_code"] = completed.returncode
                except subprocess.TimeoutExpired:
                    receipt["errors"].append("Node/Playwright selection adapter exceeded the 300-second execution budget")
                    receipt["exit_code"] = 124
                except OSError as error:
                    receipt["errors"].append(f"Cannot execute Node/Playwright selection adapter: {error}")
                    receipt["exit_code"] = 127
            after = _source_hashes(root)
            receipt["source_hashes_after"] = after
            if before != after:
                receipt["errors"].append("Controlled source bytes changed during execution")
            receipt["static_errors_after"] = check(root)
            if receipt["exit_code"] != 0:
                receipt["errors"].append("Node/Playwright selection adapter returned a nonzero exit code")
            try:
                identity, report_errors = _adapter_report(destination, root)
                receipt["adapter_report"] = identity
                receipt["errors"].extend(report_errors)
            except (OSError, ValueError, TypeError, UnicodeError) as error:
                receipt["errors"].append(f"Cannot verify Node/Playwright evidence: {error}")
            if not receipt["errors"] and not receipt["static_errors_after"]:
                receipt["status"] = "passed"
    except (OSError, ValueError, TypeError, KeyError, UnicodeError) as error:
        receipt["errors"].append(str(error))
    receipt["finished_at"] = datetime.now(timezone.utc).isoformat()
    for label, path in (("stdout", stdout_path), ("stderr", stderr_path)):
        receipt[label] = {"path": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    (destination / "execution.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--execute", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    if args.execute != (args.output is not None):
        parser.error("--execute requires --output; --output is valid only with --execute")
    try:
        if args.execute:
            result = execute(args.root, args.output)
        else:
            errors = check(args.root)
            result = {"kind": "guided_service_reading_design_static", "layer": "design_only",
                      "status": "failed" if errors else "passed", "errors": errors,
                      "product_acceptance": "not_run", "candidate_design_status": "pending_review"}
    except (OSError, ValueError) as error:
        result = {"status": "failed", "layer": "design_only", "product_acceptance": "not_run", "errors": [str(error)]}
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"Guided service/reading design: {result['status']} (design only; product acceptance not_run)")
        for error in result.get("errors", []) + result.get("static_errors", []):
            print(f"- {error}")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
