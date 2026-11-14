"""Validate independently reviewed current-source references, never product success.

Capture and review are separate. Partial reviewed catalogues cover only their
exact states; prototypes, reference metadata and guard fixtures grant no native
or product capability.
"""
from __future__ import annotations

from datetime import datetime
import copy
import hashlib
from html.parser import HTMLParser
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import tempfile
from urllib.parse import parse_qs, unquote, urljoin, urlsplit
import zlib

from gate_core import digest, safe_path

CATALOGUE = 'docs/development/current-reference-catalogue.json'
REVIEW = 'docs/development/current-reference-review.json'
RUNNER = 'scripts/report/capture_current_references.mjs'
CLIENT_RUNNER = 'scripts/report/capture_current_client_references.mjs'
CORETEXT_FONT_PROBE = '''import Foundation
import CoreText
let name = CommandLine.arguments[1]
let font = CTFontCreateWithName(name as CFString, 18, nil)
let row: [String: Any] = ["requested": name, "resolved": CTFontCopyPostScriptName(font) as String, "family": CTFontCopyFamilyName(font) as String, "url": (CTFontCopyAttribute(font, kCTFontURLAttribute) as? URL)?.path ?? ""]
let data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
print(String(data: data, encoding: .utf8)!)'''
CORETEXT_UI_FONT_PROBE = r'''import Foundation
import CoreText
import CryptoKit
func tag(_ id: UInt32) -> String {
 String(bytes:[UInt8((id>>24)&255),UInt8((id>>16)&255),UInt8((id>>8)&255),UInt8(id&255)],encoding:.ascii) ?? ""
}
func describe(_ font: CTFont) throws -> [String:Any] {
 guard let url = CTFontCopyAttribute(font,kCTFontURLAttribute) as? URL else { throw NSError(domain:"font_url_missing",code:1) }
 let data=try Data(contentsOf:url), s=try FileManager.default.attributesOfItem(atPath:url.path)
 let axes = (CTFontCopyVariationAxes(font) as? [[String:Any]] ?? []).map { a -> [String:Any] in
  let n=a[kCTFontVariationAxisIdentifierKey as String] as? NSNumber ?? 0
  return ["identifier":n,"tag":tag(n.uint32Value),"name":a[kCTFontVariationAxisNameKey as String] ?? NSNull(),"minimum":a[kCTFontVariationAxisMinimumValueKey as String] ?? NSNull(),"maximum":a[kCTFontVariationAxisMaximumValueKey as String] ?? NSNull(),"default":a[kCTFontVariationAxisDefaultValueKey as String] ?? NSNull()]
 }.sorted { ($0["identifier"] as! NSNumber).uint32Value < ($1["identifier"] as! NSNumber).uint32Value }
 let vars=(CTFontCopyVariation(font) as? [NSNumber:NSNumber] ?? [:]).map { ["identifier":$0.key,"tag":tag($0.key.uint32Value),"value":$0.value] as [String:Any] }.sorted { ($0["identifier"] as! NSNumber).uint32Value < ($1["identifier"] as! NSNumber).uint32Value }
 return ["postscript":CTFontCopyPostScriptName(font) as String,"family":CTFontCopyFamilyName(font) as String,"full_name":CTFontCopyFullName(font) as String,"size":CTFontGetSize(font),"ui_type":CTFontGetUIFontType(font).rawValue,"axes":axes,"variations":vars,"url":url.path,
 "file":["bytes":data.count,"sha256":SHA256.hash(data:data).map{String(format:"%02x",$0)}.joined(),"owner":s[.ownerAccountName] ?? NSNull(),"owner_id":s[.ownerAccountID] ?? NSNull(),"group":s[.groupOwnerAccountName] ?? NSNull(),"group_id":s[.groupOwnerAccountID] ?? NSNull(),"mode":s[.posixPermissions] ?? NSNull(),"inode":s[.systemFileNumber] ?? NSNull(),"filesystem":s[.systemNumber] ?? NSNull(),"real_path":url.resolvingSymlinksInPath().path]]
}
let input=try Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[1]))
let observations=try JSONSerialization.jsonObject(with:input) as! [[String:Any]]
var results=[[String:Any]]()
for (index,o) in observations.enumerated() {
 let font=o["platform_font"] as! [String:Any], requested=font["postScriptName"] as! String
 guard o["font_style"] as? String == "normal", o["font_stretch"] as? String == "100%", o["font_variation_settings"] as? String == "normal", ["auto","none"].contains(o["font_optical_sizing"] as? String ?? ""), let family=o["font_family"] as? String, let first=family.components(separatedBy:",").first, ["-apple-system","system-ui"].contains(first.trimmingCharacters(in:.whitespacesAndNewlines).trimmingCharacters(in:CharacterSet(charactersIn:"\""))) else {
  results.append(["index":index,"source_observation":o,"attempts":[],"exact_match":NSNull(),"status":"unsupported_context"]);continue
 }
 let size=Double((o["font_size"] as! String).dropLast(2))!,weight=Double(o["font_weight"] as! String)!
 var attempts=[[String:Any]](), match:[String:Any]?=nil
 func attempt(_ font:CTFont,_ api:String,_ parameters:[String:Any],_ base:[String:Any]) throws {
  let native=try describe(font)
  let row:[String:Any] = ["api":api,"parameters":parameters,"base_native":base,"native":native]
  attempts.append(row)
  if native["postscript"] as? String == requested {match=row}
 }
 if let system=CTFontCreateUIFontForLanguage(.system,size,nil) {
  let base=try describe(system)
  try attempt(system,"CTFontCreateUIFontForLanguage",["ui_type":"system","size":size,"language":NSNull()],base)
  if match == nil,let emphasized=CTFontCreateUIFontForLanguage(.emphasizedSystem,size,nil) {
   try attempt(emphasized,"CTFontCreateUIFontForLanguage",["ui_type":"emphasizedSystem","size":size,"language":NSNull()],try describe(emphasized))
  }
  if match == nil {
   var variations=[NSNumber:NSNumber]()
   for a in CTFontCopyVariationAxes(system) as? [[String:Any]] ?? [] {
    guard let n=a[kCTFontVariationAxisIdentifierKey as String] as? NSNumber else {continue}
    if tag(n.uint32Value)=="wght" {variations[n]=NSNumber(value:weight)}
    if tag(n.uint32Value)=="opsz" && o["font_optical_sizing"] as? String == "auto" {variations[n]=NSNumber(value:size)}
   }
   let descriptor=CTFontDescriptorCreateCopyWithAttributes(CTFontCopyFontDescriptor(system),[kCTFontVariationAttribute:variations] as CFDictionary)
   let parameters:[String:Any] = ["ui_type":"system","size":size,"language":NSNull(),"candidate_variations":variations.map{["identifier":$0.key,"value":$0.value] as [String:Any]}.sorted{($0["identifier"] as! NSNumber).uint32Value < ($1["identifier"] as! NSNumber).uint32Value}]
   try attempt(CTFontCreateWithFontDescriptor(descriptor,size,nil),"CTFontCreateWithFontDescriptor",parameters,base)
  }
 }
 results.append(["index":index,"source_observation":o,"attempts":attempts,"exact_match":match ?? NSNull(),"status":match == nil ? "unresolved" : "exact_native_match"])
}
print(String(data:try JSONSerialization.data(withJSONObject:["kind":"coretext-ui-instance-exact/v1","input_sha256":SHA256.hash(data:input).map{String(format:"%02x",$0)}.joined(),"results":results],options:[.sortedKeys]),encoding:.utf8)!)
'''
ABSENT = 'independently reviewed approved current-target reference catalogue is absent'
SHA = re.compile(r'[0-9a-f]{64}\Z')
NONCE = re.compile(r'[0-9a-f]{32}\Z')


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False)


def identity(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise ValueError('duplicate reference JSON key: ' + key)
        result[key] = value
    return result


def load(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=_unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON number')))


def _actor(value):
    return isinstance(value, str) and 0 < len(value) <= 240 and value == value.strip() and all(ord(c) >= 32 and ord(c) != 127 for c in value)


def _actors(values):
    return isinstance(values, list) and bool(values) and len(values) == len(set(values)) and all(_actor(v) for v in values)


def _date(value):
    parsed = datetime.fromisoformat(value.replace('Z', '+00:00'))
    if parsed.tzinfo is None: raise ValueError('timezone-aware reference time required')
    return parsed


def _hashed(root, row, *, artifact=False):
    keys = {'path', 'sha256', 'bytes'} if artifact else {'path', 'sha256'}
    if not isinstance(row, dict) or set(row) != keys or not isinstance(row.get('sha256'), str) or not SHA.fullmatch(row['sha256']):
        raise ValueError('exact reference file path/hash required')
    p = safe_path(root, row['path'])
    if not p.is_file() or digest(p) != row['sha256']: raise ValueError('reference raw file hash differs: ' + row['path'])
    if artifact and (type(row['bytes']) is not int or row['bytes'] <= 0 or p.stat().st_size != row['bytes']):
        raise ValueError('reference artifact byte length differs')
    return p


def descriptor(row):
    """Stable source-state input, including its actual executable recipe."""
    return {key: row[key] for key in ('id', 'scenario', 'source_case_ids', 'state',
            'mapping_status', 'design_recipe', 'target', 'fixture', 'variant_measurements')}


def _assertion_actual(step, actual):
    action = step['action']
    if action in ('assert_visible', 'assert_disabled', 'assert_focused', 'assert_design_predicate'):
        return actual is True
    if action == 'assert_absent': return type(actual) is int and actual == 0
    if action == 'assert_text_contains': return isinstance(actual, str) and isinstance(step['value'], str) and step['value'] in actual
    if action in ('assert_attribute', 'assert_property', 'assert_value'):
        return type(actual) is type(step['value']) and actual == step['value']
    raise ValueError('unregistered source assertion action: ' + action)


def _transcript_matches(observations, expected):
    return (isinstance(observations, list) and len(observations) == len(expected)
            and all(isinstance(o, dict) and set(o) == set(step) | {'step_index', 'status', 'actual'}
                    and o['status'] == 'passed' and type(o['step_index']) is int and o['step_index'] == i
                    and {k: v for k, v in o.items() if k not in ('status', 'actual', 'step_index')} == step
                    and _assertion_actual(step, o['actual']) for o, (i, step) in zip(observations, expected)))


def _action_actual(step, actual):
    """Check the fixed producer's raw action result, not only its status label."""
    if step['action'].startswith('assert_'): return _assertion_actual(step, actual)
    if not isinstance(actual, dict): return False
    action = step['action']
    if action in ('navigate', 'navigate_scenario'):
        if set(actual) != {'url', 'status'} or type(actual['status']) is not int or actual['status'] != 200: return False
        url = urlsplit(actual['url'])
        if url.scheme != 'http' or url.hostname != '127.0.0.1' or url.username or url.password: return False
        if action == 'navigate_scenario': return parse_qs(url.query).get('scenario') == [step['scenario']]
        return (unquote(url.path) == '/' + step['path']
                and parse_qs(url.query) == {k: [v] for k, v in step['query'].items()})
    if action == 'set_viewport': return actual == {'viewport': {'width': step['width'], 'height': step['height']}}
    if action == 'fill': return actual == {'actual': step['value']}
    if action == 'press': return actual == {'key': step['key']}
    if action == 'select_option': return actual == {'selected_values': [step['value']]}
    if action == 'click':
        return (set(actual) == {'clicked_selector', 'matching_count'} and actual['clicked_selector'] == step['selector']
                and type(actual['matching_count']) is int and actual['matching_count'] >= 0)
    if action == 'evaluate_design_api':
        return set(actual) == {'declared_scope', 'result'} and actual['declared_scope'] == step['scope']
    raise ValueError('unregistered source recipe action: ' + action)


def _final_navigation(recipe):
    navigation = None
    for step in recipe:
        if step['action'] == 'navigate': navigation = {'path': step['path'], 'query': dict(step['query'])}
        elif step['action'] == 'navigate_scenario':
            if navigation is None: raise ValueError('scenario navigation requires a preceding declared source route')
            navigation['query']['scenario'] = step['scenario']
    if navigation is None: raise ValueError('declared source navigation required')
    return navigation


def authoritative_context(root):
    # Lazy imports avoid installing any authority through external JSON.
    import current_design_alignment as current
    import all_page_design_coverage as pages
    from history_design import INHERITED_RUNTIME_ASSETS
    from design_authority import current_source_approval
    from current_preview_authority import affects_preview_target, PREFIX as PREVIEW_SUPPLEMENT_PREFIX
    approval = current_source_approval(root)
    if approval['errors'] or approval['status'] != 'approved_by_user': raise ValueError('actual current design source approval invalid')
    rows = [s for page in pages.expected_contract(root)['pages'] for s in page['states']]
    return {'target_id': current.TARGET, 'composition_id': current.COMPOSITION_ID,
            'design_closure_sha256': current.DESIGN_CLOSURE_SHA256,
            'design_sources': current.design_sources(root),
            'runtime_assets': dict(INHERITED_RUNTIME_ASSETS),
            'known_states': set(current.STATE_IDS),
            'descriptors': {r['id']: descriptor(r) for r in rows
                            if r['mapping_status'] in ('mapped_recipe', 'mapped_absent')
                            and (not affects_preview_target(r) or any(s.get('action') == 'navigate'
                                 and s.get('path') == PREVIEW_SUPPLEMENT_PREFIX + 'index.html'
                                 for s in r.get('design_recipe', [])))},
            'generator_sha256': digest(safe_path(root, RUNNER))}


def capture_request(root, state_ids, authors, nonce, *, clock_iso='2026-10-18T10:24:08.000Z'):
    context = authoritative_context(root)
    if not _actors(authors) or not isinstance(nonce, str) or not NONCE.fullmatch(nonce): raise ValueError('actual capture authors and nonce required')
    if not state_ids or len(state_ids) != len(set(state_ids)) or set(state_ids) - set(context['descriptors']):
        raise ValueError('capture states must have genuine executable source recipes')
    _date(clock_iso)
    return {'schema_version': 1, 'kind': 'current-design-reference-request/v1',
            **{k: context[k] for k in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets')},
            'execution_nonce': nonce, 'authors': authors, 'clock_iso': clock_iso,
            'generator': {'path': RUNNER, 'sha256': context['generator_sha256']},
            'state_descriptors': [{'state_id': sid, 'descriptor_json': canonical(context['descriptors'][sid]),
                                   'descriptor_sha256': identity(context['descriptors'][sid])} for sid in state_ids]}


def _validate(root, required, context):
    """Internal pure-context seam for negative guards; public callers use authority."""
    root = Path(root); errors, gaps, approved = [], [], set()
    result = {'schema_version': 1, 'kind': 'current-reference-assessment/v1',
              'status': 'failed', 'errors': errors, 'gaps': gaps, 'approved_state_ids': [],
              'required_state_ids': sorted(required), 'product_alignment_approved': False,
              'native_verified': False, 'catalogue': None, 'review': None}
    try:
        if not required <= context['known_states']: raise ValueError('unknown required reference state')
        cpath = safe_path(root, CATALOGUE)
        if not cpath.is_file():
            gaps.append(ABSENT); result['status'] = 'blocked'; return result
        catalogue = load(cpath)
        expected_keys = {'schema_version', 'kind', 'target_id', 'composition_id', 'design_closure_sha256',
                         'design_sources', 'runtime_assets', 'authors', 'pack_root', 'generation', 'entries'}
        if set(catalogue) != expected_keys or type(catalogue['schema_version']) is not int or catalogue['schema_version'] != 1 or catalogue['kind'] != 'current-design-reference-catalogue/v1':
            raise ValueError('typed reference catalogue schema differs')
        for key in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets'):
            if catalogue[key] != context[key]: raise ValueError('reference current-source authority differs: ' + key)
        for key in ('design_sources', 'runtime_assets'):
            for path, value in catalogue[key].items():
                if digest(safe_path(root, path)) != value: raise ValueError('reference source bytes drift: ' + path)
        if not _actors(catalogue['authors']): raise ValueError('unique capture authors required')
        pack = safe_path(root, catalogue['pack_root'])
        if not pack.is_dir(): raise ValueError('actual immutable reference generation pack required')
        generation_file = _hashed(pack, catalogue['generation']); generation = load(generation_file)
        if (type(generation.get('schema_version')) is not int or generation['schema_version'] != 1
                or generation.get('kind') != 'current-design-reference-generation/v1'
                or generation.get('collection_complete') is not True
                or generation.get('prototype_reference_only') is not True
                or generation.get('product_alignment_approved') is not False
                or generation.get('native_verified') is not False):
            raise ValueError('real reference generation identity/capability differs')
        request = load(_hashed(pack, generation['request']))
        if request.get('kind') != 'current-design-reference-request/v1' or type(request.get('schema_version')) is not int or request['schema_version'] != 1:
            raise ValueError('actual reference capture request required')
        for key in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets', 'authors'):
            if request.get(key) != catalogue[key] or generation.get(key) != catalogue[key]: raise ValueError('generation/request scope differs: ' + key)
        generator = request.get('generator')
        if generator != {'path': RUNNER, 'sha256': context['generator_sha256']} or digest(safe_path(root, RUNNER)) != context['generator_sha256']:
            raise ValueError('code-owned reference generator differs')
        if (not isinstance(generation.get('execution_nonce'), str) or not NONCE.fullmatch(generation['execution_nonce'])
                or request.get('execution_nonce') != generation['execution_nonce']): raise ValueError('actual reference generation nonce differs')
        started, finished = _date(generation['started_at']), _date(generation['finished_at'])
        if finished < started: raise ValueError('reference execution interval reversed')
        _date(request['clock_iso'])
        expected_sources = {**context['design_sources'], **context['runtime_assets']}
        if generation.get('sources_before') != expected_sources or generation.get('sources_after') != expected_sources:
            raise ValueError('generation did not bind exact original source bytes before/after')
        for key in ('page_errors', 'console_errors', 'request_errors', 'fatal_errors', 'external_requests', 'source_drift'):
            if generation.get(key) != []: raise ValueError('source replay runtime violation: ' + key)
        command = generation['command']; workspace = Path(generation['workspace_root'])
        origin = Path(generation['original_output']); reqorigin = Path(generation['original_request'])
        if (not workspace.is_absolute() or not origin.is_absolute() or not reqorigin.is_absolute()
                or not origin.is_relative_to(workspace / '.build') or reqorigin != origin / 'request.json'
                or len(command) != 6 or Path(command[0]).name != 'node'
                or command[1:] != [str(workspace / RUNNER), '--request', str(reqorigin), '--output', str(origin)]):
            raise ValueError('fixed source capture command provenance differs')
        descriptors = request['state_descriptors']
        if not isinstance(descriptors, list) or not descriptors or len({r['state_id'] for r in descriptors}) != len(descriptors): raise ValueError('exact unique captured state descriptors required')
        expected_descriptors = context['descriptors']; requested = {}
        for item in descriptors:
            sid = item['state_id']; expected = expected_descriptors.get(sid)
            if (set(item) != {'state_id', 'descriptor_json', 'descriptor_sha256'} or expected is None
                    or item['descriptor_json'] != canonical(expected) or item['descriptor_sha256'] != identity(expected)):
                raise ValueError('reference recipe/input descriptor differs: ' + sid)
            requested[sid] = item
        rows = generation['results']
        if not isinstance(rows, list) or len(rows) != len(requested) or {r['state_id'] for r in rows} != set(requested): raise ValueError('source replay omitted or duplicated a requested state')
        if len({r['state_id'] for r in rows}) != len(rows): raise ValueError('duplicate generation state')
        rows = {r['state_id']: r for r in rows}
        status = 'passed' if all(r.get('status') == 'passed' for r in rows.values()) else 'failed'
        if generation.get('status') != status: raise ValueError('generation failed rows hidden by overall status')
        entries = catalogue['entries']
        if not isinstance(entries, list) or not entries or len({r['state_id'] for r in entries}) != len(entries): raise ValueError('exact unique actual reference entries required')
        valid_entries = {}
        from reading_alignment_preflight import png_pixels, enclosing_png_size
        for entry in entries:
            if set(entry) != {'state_id', 'descriptor_sha256', 'target_png', 'full_png', 'geometry', 'assertions'}: raise ValueError('reference entry fields differ')
            sid = entry['state_id']; row = rows.get(sid)
            if (row is None or row.get('status') != 'passed' or row.get('errors') != []
                    or type(row.get('assertion_count')) is not int or row['assertion_count'] <= 0
                    or type(row.get('skipped_assertions')) is not int or row['skipped_assertions'] != 0
                    or row.get('descriptor_sha256') != requested[sid]['descriptor_sha256']
                    or entry['descriptor_sha256'] != row['descriptor_sha256']):
                raise ValueError('reference entry lacks positive actual source assertions: ' + sid)
            for kind in ('target_png', 'full_png', 'geometry', 'assertions'):
                if entry[kind] != row['artifacts'][kind]: raise ValueError('reference differs from original generated artifact: ' + sid)
                file = _hashed(pack, entry[kind], artifact=True)
                if kind.endswith('png'):
                    width, height, _pixels = png_pixels(file.read_bytes(), decode_pixels=False)
                    if row['pixel_sizes'][kind] != [width, height]: raise ValueError('actual reference PNG dimensions differ')
            observations = load(_hashed(pack, entry['assertions'], artifact=True))
            recipe = expected_descriptors[sid]['design_recipe']
            expected_assertions = [(i, step) for i, step in enumerate(recipe) if step['action'].startswith('assert_')]
            if (not _transcript_matches(observations, expected_assertions)
                    or len(observations) != row['assertion_count']):
                raise ValueError('actual assertion transcript differs')
            actions = row.get('actions')
            if (not isinstance(actions, list) or len(actions) != len(recipe)
                    or any(not isinstance(a, dict) or set(a) != {'index', 'phase', 'step', 'status', 'actual'}
                           or type(a['index']) is not int or a['index'] != i or a['phase'] != 'recipe'
                           or a['step'] != step or a['status'] != 'passed' or not _action_actual(step, a['actual'])
                           for i, (a, step) in enumerate(zip(actions, recipe)))):
                raise ValueError('actual complete recipe action transcript differs')
            for observation, (i, _step) in zip(observations, expected_assertions):
                if canonical(actions[i]['actual']) != canonical(observation['actual']):
                    raise ValueError('action and assertion observations contradict')
            final_start = len(recipe)
            while final_start and recipe[final_start - 1]['action'].startswith('assert_'): final_start -= 1
            if not _transcript_matches(row.get('post_capture_consistency'), list(enumerate(recipe))[final_start:]):
                raise ValueError('actual post-capture assertion transcript differs')
            inputs = load(_hashed(pack, row['inputs'], artifact=True))
            if (set(inputs) != {'request_descriptor', 'descriptor', 'clock_iso', 'device_scale_factor', 'original_source_identity'}
                    or inputs['request_descriptor'] != requested[sid] or inputs['descriptor'] != expected_descriptors[sid]
                    or inputs['clock_iso'] != request['clock_iso'] or type(inputs['device_scale_factor']) is not int
                    or inputs['device_scale_factor'] != 1 or inputs['original_source_identity'] != expected_sources):
                raise ValueError('actual state input descriptor/source/clock differs')
            dom = load(_hashed(pack, row['source_dom'], artifact=True))
            navigation = _final_navigation(recipe)
            url = urlsplit(dom['url'])
            if (set(dom) != {'url', 'selector', 'document_html', 'target_html'}
                    or url.scheme != 'http' or url.hostname != '127.0.0.1' or url.username or url.password
                    or unquote(url.path) != '/' + navigation['path']
                    or parse_qs(url.query) != {k: [v] for k, v in navigation['query'].items()}
                    or dom['selector'] != expected_descriptors[sid]['target']['selector']
                    or any(not isinstance(dom[k], str) or not dom[k].strip() for k in ('document_html', 'target_html'))):
                raise ValueError('raw original source DOM/route/target differs')
            geometry = load(_hashed(pack, entry['geometry'], artifact=True))
            viewport = geometry['viewport']; box = geometry['target_box']
            expected_viewport = {'width': 1600, 'height': 1100, 'deviceScaleFactor': 1}
            for step in expected_descriptors[sid]['design_recipe']:
                if step['action'] == 'set_viewport': expected_viewport.update(width=step['width'], height=step['height'])
            if (set(viewport) != {'width', 'height', 'deviceScaleFactor'} or viewport['deviceScaleFactor'] != 1
                    or type(viewport['deviceScaleFactor']) not in (int, float)
                    or viewport != expected_viewport
                    or row['pixel_sizes']['full_png'] != [viewport['width'], viewport['height']]
                    or any(type(viewport[k]) is not int or viewport[k] <= 0 for k in ('width', 'height'))
                    or any(type(box[k]) not in (float, int) or not math.isfinite(box[k]) for k in ('x', 'y', 'width', 'height'))
                    or box['width'] <= 0 or box['height'] <= 0
                    or row['pixel_sizes']['target_png'] != enclosing_png_size(box, geometry['clip']['document_scroll'])):
                raise ValueError('finite actual source geometry required')
            valid_entries[sid] = entry
        result['catalogue'] = {'path': CATALOGUE, 'sha256': digest(cpath)}
        rpath = safe_path(root, REVIEW)
        if not rpath.is_file(): gaps.append('independent current reference catalogue review is absent')
        else:
            review = load(rpath)
            if (type(review.get('schema_version')) is not int or review['schema_version'] != 1
                    or review.get('kind') != 'independent-current-reference-review/v1'):
                raise ValueError('independent reference review schema differs')
            if review.get('status') == 'pending_review': gaps.append('independent current reference catalogue review is pending')
            else:
                if (review.get('status') != 'approved' or review.get('independent_non_author_review') is not True
                        or not _actor(review.get('reviewer_id')) or review['reviewer_id'] in catalogue['authors']
                        or review.get('author_ids') != catalogue['authors']): raise ValueError('reference approval must be independent of every capture author')
                for key in ('target_id', 'composition_id', 'design_closure_sha256'):
                    if review.get(key) != context[key]: raise ValueError('review current source identity differs: ' + key)
                if (review.get('catalogue') != result['catalogue'] or review.get('source_approval_extended') is not False
                        or review.get('product_alignment_approved') is not False or review.get('native_verified') is not False
                        or _date(review['reviewed_at']) < finished): raise ValueError('review raw catalogue/time/capability differs')
                reviewed = review.get('reviewed_entries')
                if (not isinstance(reviewed, dict) or not reviewed or not set(reviewed) <= set(valid_entries)
                        or any(v != identity(valid_entries[sid]) for sid, v in reviewed.items())): raise ValueError('exact independently reviewed entry digests required')
                approved = set(reviewed); result['review'] = {'path': REVIEW, 'sha256': digest(rpath), 'reviewer_id': review['reviewer_id']}
        missing = required - approved
        if missing: gaps.append('independently reviewed current reference states missing: ' + ','.join(sorted(missing)))
        result['approved_state_ids'] = sorted(approved)
        result['status'] = 'blocked' if gaps else 'passed'
    except (OSError, ValueError, TypeError, KeyError, AttributeError, IndexError, StopIteration, struct.error, zlib.error) as error:
        errors.append('current reference catalogue invalid: ' + str(error)); result['status'] = 'failed'
    return result


GENERAL_LINEAGE = 'apps/desktop/tests/fixtures/general-visual/legacy-general-descriptors.json'
GENERAL_LINEAGE_SHA256 = '45d16a2ef9970b4bdd2f186d942df141f005786f4f8daec31eb50d71cd25ba27'


def _legacy_context(root, context):
    """Validate the immutable old pack without granting its retired General source.

    The old generation includes General normal, so its entire signed provenance
    must still be checked against that original descriptor. This pinned lineage
    cannot supply a current General recipe, reference or approved state.
    """
    from current_general_visual_authority import STATE_IDS as general_ids
    if not set(context['descriptors']) & set(general_ids):
        return context
    path = safe_path(root, GENERAL_LINEAGE)
    if digest(path) != GENERAL_LINEAGE_SHA256:
        raise ValueError('frozen historical General descriptors changed')
    lineage = load(path)
    rows = lineage.get('descriptors', [])
    if (lineage.get('kind') != 'frozen-general-guided-descriptor-lineage/v1'
            or len(rows) != len(general_ids) or {r['id'] for r in rows} != set(general_ids)):
        raise ValueError('exact historical General14 descriptor lineage required')
    value = {**context, 'descriptors': dict(context['descriptors'])}
    value['descriptors'].update({row['id']: descriptor(row) for row in rows})
    return value


def _assess_legacy(root, required):
    root = Path(root)
    # Keep the original accurate absent result without requiring an uninstalled
    # generator. A real catalogue invokes the full independently owned resolver.
    if not safe_path(root, CATALOGUE).is_file():
        return {'schema_version': 1, 'kind': 'current-reference-assessment/v1', 'status': 'blocked',
                'errors': [], 'gaps': [ABSENT], 'approved_state_ids': [],
                'product_alignment_approved': False, 'native_verified': False, 'catalogue': None, 'review': None}
    try:
        context = _legacy_context(root, authoritative_context(root))
        result = _validate(root, required, context)
        from current_general_visual_authority import STATE_IDS as general_ids
        # Even an independently reviewed old General entry is lineage only.
        result['approved_state_ids'] = sorted(set(result['approved_state_ids']) - set(general_ids))
        return result
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        return {'schema_version': 1, 'kind': 'current-reference-assessment/v1', 'status': 'failed',
                'errors': ['current reference authority invalid: ' + str(error)], 'gaps': [],
                'approved_state_ids': [], 'product_alignment_approved': False, 'native_verified': False}


def _merge_assessments(required, assessments):
    """Combine exact disjoint authorities without promoting either capability."""
    result = {'schema_version': 1, 'kind': 'current-reference-assessment/v1',
              'status': 'blocked', 'errors': [], 'gaps': [], 'approved_state_ids': [],
              'required_state_ids': sorted(required), 'product_alignment_approved': False,
              'native_verified': False, 'catalogue': None, 'review': None,
              'assessments': assessments}
    approved = set()
    for name, (wanted, value) in assessments.items():
        result['errors'].extend(value['errors'])
        result['gaps'].extend(value['gaps'])
        # Do not let an authority's larger reviewed catalogue leak across scope.
        covered = set(value['approved_state_ids']) & wanted
        approved.update(covered)
        missing = wanted - covered
        if missing and not value['errors'] and not value['gaps']:
            result['gaps'].append(name + ' reference authority omitted required states: ' + ','.join(sorted(missing)))
    result['approved_state_ids'] = sorted(approved)
    result['status'] = ('failed' if result['errors'] else 'blocked' if result['gaps']
                        or approved != required else 'passed')
    # JSON-safe, complete subreceipts retain each original catalogue and review.
    result['assessments'] = {name: value for name, (_wanted, value) in assessments.items()}
    return result


def assess(root, required=None):
    # Current General and original guided authorities are always disjoint,
    # including a full-product request. Global AST validation remains inside the
    # original authority resolver and is never replaced by local General success.
    from current_general_visual_authority import STATE_IDS as general_ids
    from current_design_alignment import STATE_IDS as all_ids
    from current_general_reference import assess as assess_general
    wanted = set(all_ids if required is None else required)
    if not wanted or not wanted <= set(all_ids):
        return {'schema_version': 1, 'kind': 'current-reference-assessment/v1', 'status': 'failed',
                'errors': ['nonempty known current reference scope required'], 'gaps': [],
                'approved_state_ids': [], 'required_state_ids': sorted(wanted),
                'product_alignment_approved': False, 'native_verified': False,
                'catalogue': None, 'review': None}
    general = wanted & set(general_ids)
    other = wanted - set(general_ids)
    if not other:
        return assess_general(root, general)
    assessments = {'non_general': (other, _assess_legacy(root, other))}
    if general:
        assessments['general'] = (general, assess_general(root, general))
    return _merge_assessments(wanted, assessments)


def reference_gaps(root, required=None):
    result = assess(root, required)
    return result['errors'] + result['gaps']


# V2 is a separate authority path. No v1 catalogue, generator hash, reviewed
# entry or target PNG is rewritten or implicitly granted a larger scope.
CLIENT_SURFACES = {
    'main': ('#main-window', '.main-layout', (1040, 720), (760, 560)),
    'settings': ('#settings-window', '#settings-content', (1040, 760), (760, 560)),
    'hud': ('#hud-window', '#hud-source', (660, 280), (360, 140)),
    'learning': ('#learning-window', '#learning-content', (600, 670), (390, 480)),
}


def client_surface(role, variant='normal'):
    """Code-owned CSS-pixel contexts; these are not native geometry evidence."""
    if role not in CLIENT_SURFACES or variant not in ('normal', 'narrow'):
        raise ValueError('unknown client role or viewport variant')
    selector, content, normal, narrow = CLIENT_SURFACES[role]
    width, height = normal if variant == 'normal' else narrow
    return {'role': role, 'variant': variant, 'selector': selector,
            'content_selector': content, 'viewport': {'width': width, 'height': height,
            'device_scale_factor': 1}, 'css_zoom': 1,
            'projection': 'original_surface_coordinate_only/v1',
            'capture_scope': 'whole_client', 'native_attestation': False,
            'native_controls_material_verified': False}


def client_browser_identity(root):
    """Read this repository's installed PW and its real managed browser binary.

    Neither a path/version supplied by a capture nor an external catalogue can
    choose another executable. The binary itself supplies its version.
    """
    script = '''const {createRequire}=require('node:module');const {readFileSync}=require('node:fs');const {createHash}=require('node:crypto');const {execFileSync}=require('node:child_process');const use=createRequire(process.argv[1]);const {chromium}=use('playwright-core');const executable=chromium.executablePath();const bytes=readFileSync(executable);const output=execFileSync(executable,['--version'],{encoding:'utf8',timeout:15000});const found=output.match(/(\\d+\\.\\d+\\.\\d+\\.\\d+)/);if(!found)throw Error('actual Chromium binary version missing');process.stdout.write(JSON.stringify({version:found[1],executable,sha256:createHash('sha256').update(bytes).digest('hex'),playwright:use('playwright-core/package.json').version}));'''
    try:
        result = subprocess.run(['node', '-e', script, str(Path(root) / 'package.json')],
                                cwd=root, text=True, capture_output=True, timeout=25, check=True)
        value = json.loads(result.stdout, object_pairs_hook=_unique)
        lock = load(Path(root) / 'package-lock.json')
        package = load(Path(root) / 'package.json')
        locked_version = lock['packages']['node_modules/playwright-core']['version']
        declared_version = package['devDependencies']['playwright-core']
        if value['playwright'] != locked_version or locked_version != declared_version:
            raise ValueError('installed client PW differs from exact repository lock')
        manifest = load(Path(root) / 'node_modules/playwright-core/browsers.json')
        chromium = next(row for row in manifest['browsers'] if row['name'] == 'chromium')
        if value['version'] != chromium['browserVersion']:
            raise ValueError('actual client browser differs from managed PW browser manifest')
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, StopIteration) as error:
        raise ValueError('actual managed client browser identity unavailable') from error
    _client_shape(value, ('version', 'executable', 'sha256', 'playwright'), 'actual client browser schema differs')
    return value


def client_capture_request(root, state_ids, authors, nonce, *, role, variant='normal',
                           clock_iso='2026-10-18T10:24:08.000Z'):
    root = Path(root)
    request = capture_request(root, state_ids, authors, nonce, clock_iso=clock_iso)
    surface = client_surface(role, variant)
    for item in request['state_descriptors']:
        if load_descriptor(item)['target']['container'] != role:
            raise ValueError('state belongs to a different client role')
    request.update(schema_version=2, kind='current-client-reference-request/v2',
                   client_surface=surface,
                   browser=client_browser_identity(root),
                   generator={'path': CLIENT_RUNNER, 'sha256': digest(root / CLIENT_RUNNER)})
    return request



CLIENT_C1_APPROVAL = 'docs/development/design-approval-client-layout-20261105.json'
CLIENT_C1_APPROVAL_SHA256 = 'ce8071da9fa6b429703ad3a658cef98de17047e2fdad01ed6fbd79ed1ebe0f01'
CLIENT_C1_CONTRACT = 'docs/design/first-product-v1/client-layout-20261105/contract.json'


def client_c1_authority(root):
    """Pinned user decision, original 61 sources and exact incremental bytes."""
    root = Path(root).resolve(); approval_file = safe_path(root, CLIENT_C1_APPROVAL)
    if digest(approval_file) != CLIENT_C1_APPROVAL_SHA256:
        raise ValueError('C1 approval bytes differ from reviewed user decision')
    approval = load(approval_file)
    if (approval.get('status') != 'approved_by_user' or approval.get('kind') != 'user-approved-client-container-supplement/v1'
            or approval.get('user_confirmation', {}).get('answer') != '设置改用A1000×720，其余采用'):
        raise ValueError('C1 user approval scope differs')
    parent = approval['parent_approval']; parent_path = _hashed(root, {'path': parent['path'], 'sha256': parent['sha256']})
    original = load(parent_path)
    if (parent_path.stat().st_size != parent['bytes'] or parent['source_count'] != 61
            or original.get('design_source_count') != 61 or len(original['design_sources']) != 61
            or original.get('status') != 'approved_by_user'
            or original.get('target_id') != approval['target_id']
            or original.get('composition_id') != approval['composition_id']):
        raise ValueError('C1 parent 61-source approval differs')
    for path, sha in original['design_sources'].items():
        _hashed(root, {'path': path, 'sha256': sha})
    sources = approval['approved_sources']
    if len(sources) != 3: raise ValueError('exact C1 incremental three sources required')
    for path, value in sources.items(): _hashed(root, {'path': path, **value}, artifact=True)
    contract = load(safe_path(root, CLIENT_C1_CONTRACT))
    if (contract.get('kind') != 'approved-client-container-contract/v1'
            or contract.get('approval_record') != CLIENT_C1_APPROVAL
            or contract.get('required_query') != {'settingsSize': 'native'}
            or contract['roles']['settings']['normal']['viewport'] != {'width': 1000, 'height': 720}
            or 'settings_B_normal_1040x760' not in approval['excluded_variants']):
        raise ValueError('C1 settings A or required native query contract differs')
    return {'approval': {'path': CLIENT_C1_APPROVAL, 'sha256': digest(approval_file), 'bytes': approval_file.stat().st_size},
            'parent_approval': parent, 'parent_design_closure_sha256': original['design_closure_sha256'],
            'parent_design_sources': original['design_sources'],
            'sources': sources, 'contract': contract}


def client_c1_surface(root, role, variant='normal'):
    authority = client_c1_authority(root)
    if role not in ('main', 'settings') or variant not in ('normal', 'narrow'):
        raise ValueError('unsupported approved C1 client role/variant')
    value = authority['contract']['roles'][role][variant]
    surface = client_surface(role, variant)
    surface.update(viewport={**value['viewport'], 'device_scale_factor': 1},
                   projection='approved_client_container_c1/v1', layout_contract_id=authority['contract']['id'])
    return surface


def client_c1_recipe(descriptor, authority):
    recipe = copy.deepcopy(descriptor['design_recipe'])
    for step in recipe:
        if step['action'] == 'navigate': step['query'].update(authority['contract']['required_query'])
    if descriptor['target']['container'] == 'settings' and _final_navigation(recipe)['query'].get('scenario') != 'settings':
        raise ValueError('C1 bootstrap needs an explicit settings source scenario')
    return recipe


def client_c1_capture_request(root, state_ids, authors, nonce, *, role, variant='normal',
                              clock_iso='2026-10-18T10:24:08.000Z'):
    request = client_capture_request(root, state_ids, authors, nonce, role=role, variant=variant, clock_iso=clock_iso)
    authority = client_c1_authority(root)
    if (request['design_sources'] != authority['parent_design_sources']
            or request['design_closure_sha256'] != authority['parent_design_closure_sha256']):
        raise ValueError('C1 current source does not equal approved parent 61-source closure')
    for item in request['state_descriptors']: client_c1_recipe(load_descriptor(item), authority)
    request.update(schema_version=3, kind='current-client-reference-request/v3',
                   client_surface=client_c1_surface(root, role, variant), approved_layout=authority)
    return request


def load_descriptor(item):
    value = json.loads(item['descriptor_json'], object_pairs_hook=_unique)
    if item['descriptor_json'] != canonical(value) or item['descriptor_sha256'] != identity(value):
        raise ValueError('canonical client descriptor identity differs')
    return value


def _client_shape(value, expected_keys, message):
    if not isinstance(value, dict) or set(value) != set(expected_keys):
        raise ValueError(message)


def _client_artifact(root, pack, value):
    path = _hashed(pack, value, artifact=True)
    if not path.is_relative_to(root / '.build'):
        raise ValueError('client evidence must remain in ignored immutable pack')
    return path


def client_projection_css(selector):
    return selector + '{position:fixed!important;left:0!important;top:0!important;right:auto!important;bottom:auto!important;transform:none!important}'


class _ClientHtml(HTMLParser):
    def __init__(self, data):
        super().__init__(); self.styles = []; self.links = []; self.active = False
        self.executable = False; self.feed(data)

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if tag == 'style': self.active = True; self.styles.append('')
        if tag == 'link' and 'stylesheet' in (attrs.get('rel') or '').split():
            self.links.append(attrs)
        if tag == 'script' or any(k.lower().startswith('on') for k in attrs): self.executable = True

    def handle_endtag(self, tag):
        if tag == 'style': self.active = False

    def handle_data(self, value):
        if self.active: self.styles[-1] += value


class _ClientTree(HTMLParser):
    """Independent inert tree, used to verify the exact projection transform."""
    VOID = frozenset(('area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr'))

    def __init__(self, source):
        super().__init__(convert_charrefs=True); self.root = {'tag': '', 'attrs': {}, 'children': []}
        self.stack = [self.root]; self.feed(source); self.close()

    def handle_starttag(self, tag, attrs):
        if len(attrs) != len({name for name, _ in attrs}):
            raise ValueError('duplicate client DOM attributes are not canonical browser serialization')
        node = {'tag': tag, 'attrs': dict(attrs), 'children': []}
        self.stack[-1]['children'].append(node)
        if tag not in self.VOID: self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in self.VOID: self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index]['tag'] == tag:
                self.stack = self.stack[:index]; return

    def handle_data(self, text):
        # The HTML document has no rendered text sibling of its html element.
        # Browser serialization drops doctype separator whitespace.
        if text and not (len(self.stack) == 1 and not text.strip()):
            children = self.stack[-1]['children']
            if children and isinstance(children[-1], str): children[-1] += text
            else: children.append(text)


def validate_client_projection(raw, derived, surface, *, approved_layout=None):
    """Reconstruct only the approved coordinate transform, never arbitrary CSS."""
    original = _ClientTree(raw['document_html']).root
    expected = copy.deepcopy(original)
    def nodes(node):
        if isinstance(node, dict):
            yield node
            for child in node['children']: yield from nodes(child)
    candidates = [n for n in nodes(expected) if n['attrs'].get('id') == surface['selector'][1:]]
    if len(candidates) != 1: raise ValueError('exact original client surface tree required')
    target = candidates[0]
    def chain(node):
        if node is target: return [node]
        for child in node['children']:
            if isinstance(child, dict):
                found = chain(child)
                if found: return [node, *found]
        return None
    ancestry = chain(expected)
    for index in range(len(ancestry) - 1, 0, -1):
        child, parent = ancestry[index], ancestry[index - 1]
        if parent['tag'] == 'html': break
        parent['children'] = [n for n in parent['children'] if not isinstance(n, dict) or n is child or n['tag'] == 'style']
    heads = [n for n in nodes(expected) if n['tag'] == 'head']
    if len(heads) != 1: raise ValueError('original client document head missing')
    head = heads[0]
    original_bases = [n for n in nodes(original) if n['tag'] == 'base']
    expected_base = urljoin(raw['url'], original_bases[0]['attrs'].get('href', '') if original_bases else '')
    if raw.get('document_base_uri') != expected_base:
        raise ValueError('original client document base URI differs')
    def strip(node):
        node['attrs'] = {k: v for k, v in node['attrs'].items() if not k.lower().startswith('on')}
        node['children'] = [n for n in node['children'] if not isinstance(n, dict) or n['tag'] not in ('script', 'base')]
        for child in node['children']:
            if isinstance(child, dict): strip(child)
    strip(expected)
    head['children'].insert(0, {'tag': 'base', 'attrs': {'href': expected_base}, 'children': []})
    if approved_layout is None:
        head['children'].append({'tag': 'style', 'attrs': {'data-client-projection': 'coordinate-only-v1'},
                                 'children': [client_projection_css(surface['selector'])]})
    else:
        if surface['projection'] != 'approved_client_container_c1/v1':
            raise ValueError('C1 cannot be mislabeled as coordinate-only')
        html_nodes = [n for n in nodes(expected) if n['tag'] == 'html']
        if len(html_nodes) != 1: raise ValueError('exact C1 html root required')
        query = parse_qs(urlsplit(raw['url']).query)
        if query.get('settingsSize') != ['native']:
            raise ValueError('C1 required settingsSize=native missing')
        role = 'settings' if query.get('scenario') == ['settings'] else 'main'
        if role != surface['role']: raise ValueError('C1 bootstrap role differs from selected surface')
        html_nodes[0]['attrs'].update({'data-client-candidate': 'c1', 'data-client-role': role, 'data-client-settings': 'native'})
        head['children'].append({'tag': 'style', 'attrs': {'data-client-projection': 'approved-client-c1-v1'},
                                 'children': [approved_layout['css']]})
    def merge_text(node):
        children = []
        for child in node['children']:
            if isinstance(child, dict): merge_text(child)
            if isinstance(child, str) and children and isinstance(children[-1], str): children[-1] += child
            else: children.append(child)
        node['children'] = children
    merge_text(expected)
    if expected != _ClientTree(derived['document_html']).root:
        raise ValueError('derived client DOM differs from approved C1 original projection' if approved_layout is not None else 'derived client DOM differs from coordinate-only original projection')


def validate_client_facts(facts, surface):
    """Reject incomplete effective facts instead of trusting an eligible label."""
    required = {'viewport', 'device_pixel_ratio', 'css_zoom', 'surface_box', 'content_box',
                'fonts', 'text_runs', 'lexical', 'forms', 'focus', 'running_animations',
                'capture_scope', 'native_attestation'}
    _client_shape(facts, required, 'complete effective client render facts required')
    vp = surface['viewport']
    if (facts['viewport'] != {k: vp[k] for k in ('width', 'height')}
            or type(facts['device_pixel_ratio']) not in (int, float)
            or facts['device_pixel_ratio'] != vp['device_scale_factor']
            or type(facts['css_zoom']) not in (int, float) or facts['css_zoom'] != surface['css_zoom']
            or facts['capture_scope'] != 'whole_client' or facts['native_attestation'] is not False):
        raise ValueError('observed logical client viewport/scope differs')
    box = facts['surface_box']
    _client_shape(box, ('x', 'y', 'width', 'height'), 'finite whole client box required')
    if any(type(v) not in (int, float) or not math.isfinite(v) for v in box.values()):
        raise ValueError('finite whole client box required')
    if box != {'x': 0, 'y': 0, 'width': vp['width'], 'height': vp['height']}:
        raise ValueError('client_geometry_mismatch: no resize is authorized')
    content = facts['content_box']
    _client_shape(content, ('x', 'y', 'width', 'height'), 'client content geometry missing')
    if (any(type(v) not in (int, float) or not math.isfinite(v) for v in content.values())
            or content['width'] <= 0 or content['height'] <= 0):
        raise ValueError('finite positive client content geometry required')
    validate_client_typography(facts)
    lexical = facts['lexical']
    _client_shape(lexical, ('ruby', 'rt', 'tokens', 'pos'), 'actual lexical render facts missing')
    if any(not isinstance(v, list) or any(not isinstance(t, str) for t in v) for v in lexical.values()):
        raise ValueError('typed actual lexical facts required')
    if not isinstance(facts['forms'], list): raise ValueError('live form properties missing')
    for form in facts['forms']:
        _client_shape(form, ('index', 'tag', 'type', 'value', 'checked', 'selected', 'disabled'),
                      'live form property schema differs')
        if (type(form['index']) is not int or form['index'] < 0
                or form['tag'] not in ('INPUT', 'SELECT', 'TEXTAREA', 'BUTTON')
                or any(type(form[k]) is not bool for k in ('checked', 'disabled'))
                or not isinstance(form['selected'], list)
                or any(not isinstance(v, str) for v in form['selected'])
                or not isinstance(form['type'], str) or not isinstance(form['value'], str)):
            raise ValueError('typed live form properties required')
    if (len({r['index'] for r in facts['forms']}) != len(facts['forms'])
            or facts['focus'] is not None and (type(facts['focus']) is not int
                 or facts['focus'] not in {r['index'] for r in facts['forms']})):
        raise ValueError('actual form/focus identity differs')
    if type(facts['running_animations']) is not int or facts['running_animations'] != 0:
        raise ValueError('active animation cannot be qualified')



def validate_client_typography(facts):
    """Font validation alone cannot waive surface geometry."""
    fonts = facts['fonts']
    _client_shape(fonts, ('status', 'checks', 'platform', 'files', 'gaps', *(['observations'] if 'observations' in fonts else [])), 'actual client font readiness missing')
    if (fonts['status'] != 'loaded' or not isinstance(fonts['checks'], list) or not fonts['checks']
            or any(set(row) != {'font', 'text', 'loaded'} or not isinstance(row['font'], str)
                   or not row['font'] or not isinstance(row['text'], str) or not row['text']
                   or row['loaded'] is not True for row in fonts['checks'])):
        raise ValueError('effective client fonts are not proven loaded')
    if not isinstance(fonts['platform'], list) or not fonts['platform'] or fonts['gaps'] != []:
        raise ValueError('resolved client font bytes are not attested')
    for row in fonts['platform']:
        _client_shape(row, ('familyName', 'postScriptName', 'isCustomFont', 'glyphCount'), 'actual platform font schema differs')
        if (not isinstance(row['familyName'], str) or not row['familyName']
                or not isinstance(row['postScriptName'], str) or not row['postScriptName']
                or type(row['isCustomFont']) is not bool or type(row['glyphCount']) is not int or row['glyphCount'] <= 0):
            raise ValueError('actual platform font facts missing')
    files = fonts['files']
    if (not isinstance(files, list) or len(files) != len(fonts['platform'])
            or {f.get('postscript_name') for f in files} != {f['postScriptName'] for f in fonts['platform']}):
        raise ValueError('all resolved client font files required')
    for row in files:
        _client_shape(row, ('postscript_name', 'original_path', 'file', 'resolution', 'coretext_probe', *(['ui_probe'] if row.get('resolution') == 'coretext_ui_instance_exact' else [])), 'resolved font file origin missing')
        if not isinstance(row['original_path'], str) or not Path(row['original_path']).is_absolute():
            raise ValueError('actual resolved font origin required')
    if not isinstance(facts['text_runs'], list) or not facts['text_runs']:
        raise ValueError('effective client typography missing')
    for row in facts['text_runs']:
        _client_shape(row, ('text', 'tag', 'font_family', 'font_size', 'font_weight', 'line_height',
                           'color', 'display', 'visibility', *(['selector', 'font_style', 'font_stretch', 'font_optical_sizing', 'font_variation_settings'] if 'observations' in fonts else [])), 'effective text style schema differs')
        if (any(not isinstance(v, str) or not v for v in row.values())
                or row['display'] == 'none' or row['visibility'] == 'hidden'
                or not re.fullmatch(r'\d+(?:\.\d+)?px', row['font_size'])
                or float(row['font_size'][:-2]) <= 0):
            raise ValueError('visible effective typography facts required')


def _font_inventory_matches(rows, name):
    return [item for item in rows if item.get('enabled') == 'yes' and item.get('valid') == 'yes'
            and any(face.get('_name') == name and face.get('enabled') == 'yes' and face.get('valid') == 'yes'
                    for face in item.get('typefaces', []))]


def _ui_observation(observation):
    keys = ('selector', 'text', 'font_family', 'font_size', 'font_weight', 'font_style', 'font_stretch',
            'font_optical_sizing', 'font_variation_settings', 'platform_font')
    _client_shape(observation, keys, 'complete observed font context required')
    if (any(not isinstance(observation[k], str) or not observation[k] for k in keys[:-1])
            or not re.fullmatch(r'\d+(?:\.\d+)?px', observation['font_size'])
            or not 0 < float(observation['font_size'][:-2]) <= 512
            or not re.fullmatch(r'\d+(?:\.\d+)?', observation['font_weight'])
            or not 1 <= float(observation['font_weight']) <= 1000):
        raise ValueError('unknown observed font context')
    font = observation['platform_font']
    _client_shape(font, ('familyName', 'postScriptName', 'isCustomFont', 'glyphCount'), 'actual observed platform font required')
    if (font['isCustomFont'] is not False or type(font['glyphCount']) is not int or font['glyphCount'] <= 0
            or any(not isinstance(font[k], str) or not font[k] for k in ('familyName', 'postScriptName'))):
        raise ValueError('actual observed platform font required')


def _ui_candidate_context(observation):
    _ui_observation(observation)
    if (observation['font_style'] != 'normal' or observation['font_stretch'] != '100%'
            or observation['font_optical_sizing'] not in ('auto', 'none')
            or observation['font_variation_settings'] != 'normal'
            or observation['font_family'].split(',')[0].strip().strip('"') not in ('-apple-system', 'system-ui')):
        raise ValueError('unknown public UI font context; no inferred instance allowed')


def _actual_font_command(command):
    completed = subprocess.run(command, capture_output=True, text=True, timeout=90)
    if completed.returncode != 0:
        raise ValueError('independent public UI font command failed: ' + completed.stderr[-800:])
    return completed.stdout


def replay_ui_font_probe(observations):
    """Compile controlled public API code afresh, never execute a supplied binary."""
    for observation in observations: _ui_candidate_context(observation)
    with tempfile.TemporaryDirectory(prefix='luna-font-replay-') as directory:
        directory = Path(directory); source = directory / 'probe.swift'; binary = directory / 'probe'
        source.write_text(CORETEXT_UI_FONT_PROBE)
        input_file = directory / 'input.json'
        # Retain JS pretty-print input as bytes; its hash is checked separately.
        input_file.write_text(json.dumps(observations, ensure_ascii=False, indent=2) + '\n')
        sdk = _actual_font_command(['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path']).strip()
        _actual_font_command(['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '-sdk', sdk, str(source), '-o', str(binary)])
        return json.loads(_actual_font_command([str(binary), str(input_file)]), object_pairs_hook=_unique)


def validate_ui_font_result(result, observations, files, inventory_rows):
    """Validate native values, including actual axes (CSS is only a candidate)."""
    if (result.get('kind') != 'coretext-ui-instance-exact/v1'
            or not isinstance(result.get('results'), list) or len(result['results']) != len(observations)):
        raise ValueError('actual public UI font result closure differs')
    for i, (row, observation) in enumerate(zip(result['results'], observations)):
        _ui_candidate_context(observation)
        if (row.get('index') != i or row.get('source_observation') != observation
                or row.get('status') != 'exact_native_match' or not isinstance(row.get('exact_match'), dict)):
            raise ValueError('unresolved public UI font instance')
        match = row['exact_match']; native = match.get('native', {}); base = match.get('base_native', {})
        if (match not in row.get('attempts', [])
                or match.get('api') not in ('CTFontCreateUIFontForLanguage', 'CTFontCreateWithFontDescriptor')
                or native.get('postscript') != observation['platform_font']['postScriptName']
                or native.get('size') != float(observation['font_size'][:-2])
                or not isinstance(native.get('axes'), list) or not isinstance(native.get('variations'), list)
                or not native.get('url') or base.get('url') != native['url']
                or native.get('ui_type') not in (2, 3) and match.get('api') == 'CTFontCreateUIFontForLanguage'):
            raise ValueError('CoreText fallback or observed public UI instance conflict')
        inventory = _font_inventory_matches(inventory_rows, base.get('postscript'))
        if native['url'] not in {item.get('path') for item in inventory}:
            raise ValueError('public UI base font owner differs from runtime inventory')
        file = files.get(native['postscript']); actual = native.get('file', {})
        if (file is None or file['original_path'] != native['url'] or actual.get('real_path') != native['url']
                or actual.get('sha256') != file['file']['sha256'] or actual.get('bytes') != file['file']['bytes']
                or actual.get('owner_id') != 0 or actual.get('group_id') != 0
                or type(actual.get('mode')) is not int or actual['mode'] & 0o022
                or not isinstance(actual.get('owner'), str) or not actual['owner']
                or not isinstance(actual.get('group'), str) or not actual['group']):
            raise ValueError('public UI font actual file bytes or owner differ')
        axes = {a.get('identifier'): a for a in native['axes']}
        if len(axes) != len(native['axes']): raise ValueError('duplicate native font axis')
        for variation in native['variations']:
            axis = axes.get(variation.get('identifier'))
            if (axis is None or variation.get('tag') != axis.get('tag')
                    or type(variation.get('value')) not in (int, float)
                    or not axis['minimum'] <= variation['value'] <= axis['maximum']):
                raise ValueError('actual native variation axis differs')


def validate_client_font_evidence(root, pack, generation, facts):
    """Font-only audit does not waive client geometry, source or approval checks."""
    raw_root = Path(root); pack = safe_path(raw_root.resolve(), Path(pack).relative_to(raw_root).as_posix())
    root = raw_root.resolve()
    validate_client_typography(facts)
    fonts = facts['fonts']; inventory = load(_client_artifact(root, pack, generation['font_inventory']))
    if (inventory.get('command') != ['system_profiler', 'SPFontsDataType', '-json']
            or type(inventory.get('exit_code')) is not int or inventory['exit_code'] != 0
            or inventory.get('error') is not None):
        raise ValueError('actual runtime font inventory command required')
    inventory_rows = json.loads(inventory['stdout'], object_pairs_hook=_unique)['SPFontsDataType']
    observations = fonts.get('observations')
    if observations is not None:
        if not isinstance(observations, list) or not observations: raise ValueError('font observation closure required')
        aggregate = {}; unique = set()
        text = {r.get('selector'): r for r in facts['text_runs']}
        for observation in observations:
            _ui_observation(observation)
            run = text.get(observation['selector'])
            if run is None or any(observation[k] != run.get(k) for k in observation if k != 'platform_font'):
                raise ValueError('font observation differs from measured text context')
            font = observation['platform_font']; key = (observation['selector'], font['postScriptName'], font['familyName'])
            if key in unique: raise ValueError('duplicate platform font observation')
            unique.add(key); aggregate_key = (font['postScriptName'], font['familyName'])
            aggregate.setdefault(aggregate_key, {**font, 'glyphCount': 0})['glyphCount'] += font['glyphCount']
        if sorted(aggregate.values(), key=lambda f: f['postScriptName']) != fonts['platform']:
            raise ValueError('font observation glyph closure differs')
    replayed = set()
    for font in fonts['files']:
        font_path = _client_artifact(root, pack, font['file'])
        if font_path.read_bytes()[:4] not in (b'\x00\x01\x00\x00', b'OTTO', b'ttcf', b'wOFF', b'wOF2', b'true'):
            raise ValueError('raw original font bytes missing')
        matches = _font_inventory_matches(inventory_rows, font['postscript_name'])
        if font['resolution'] == 'coretext_ui_instance_exact':
            if observations is None or font['coretext_probe'] is not None:
                raise ValueError('actual UI font observations required')
            reference = font['ui_probe']; key = reference['sha256']
            if key in replayed: continue
            proof = load(_client_artifact(root, pack, reference))
            source = _client_artifact(root, pack, proof['source']); binary = _client_artifact(root, pack, proof['executable'])
            input_file = _client_artifact(root, pack, proof['input'])
            compilation = load(_client_artifact(root, pack, proof['compilation']))
            environment = load(_client_artifact(root, pack, proof['environment']))
            original = Path(generation['original_output'])
            command = compilation.get('command')
            if (proof.get('kind') != 'coretext-ui-font-execution/v1' or source.read_text() != CORETEXT_UI_FONT_PROBE
                    or command != ['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '-sdk', environment['commands'][1]['stdout'].strip(), str(original / proof['source']['path']), '-o', str(original / proof['executable']['path'])]
                    or type(compilation.get('exit_code')) is not int or compilation['exit_code'] != 0 or compilation.get('error') is not None
                    or proof['execution'].get('command') != [str(original / proof['executable']['path']), str(original / proof['input']['path'])]
                    or type(proof['execution'].get('exit_code')) is not int or proof['execution']['exit_code'] != 0 or proof['execution'].get('error') is not None
                    or not binary.stat().st_mode & 0o111):
                raise ValueError('actual controlled public UI font compilation/execution missing')
            expected_commands = [['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '--version'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version'], ['/usr/bin/sw_vers']]
            if (environment.get('sdk_selection') != 'macosx' or not isinstance(environment.get('commands'), list)
                    or [r.get('command') for r in environment['commands']] != expected_commands
                    or any(type(r.get('exit_code')) is not int or r['exit_code'] != 0 or r.get('error') is not None or not r.get('stdout', '').strip() for r in environment['commands'])):
                raise ValueError('actual public UI font SDK/system identity missing')
            for row in environment['commands']:
                if _actual_font_command(row['command']) != row['stdout']:
                    raise ValueError('public UI font SDK/system environment changed')
            sdk = Path(environment['commands'][1]['stdout'].strip())
            header = sdk / 'System/Library/Frameworks/CoreText.framework/Headers/CTFont.h'
            authority = environment.get('sdk_authority', {})
            if (authority != {'path': str(header), 'sha256': digest(header), 'bytes': header.stat().st_size}
                    or 'CTFontCreateUIFontForLanguage()' not in header.read_text()):
                raise ValueError('public UI font SDK authority differs')
            inputs = load(input_file)
            expected_inputs = [o for o in observations if not _font_inventory_matches(inventory_rows, o['platform_font']['postScriptName'])]
            if inputs != expected_inputs or not inputs: raise ValueError('public UI input observation closure differs')
            recorded = json.loads(proof['execution']['stdout'], object_pairs_hook=_unique)
            if recorded.get('input_sha256') != digest(input_file): raise ValueError('public UI raw input hash differs')
            files = {f['postscript_name']: f for f in fonts['files'] if f['resolution'] == 'coretext_ui_instance_exact' and f['ui_probe'] == reference}
            validate_ui_font_result(recorded, inputs, files, inventory_rows)
            actual = replay_ui_font_probe(inputs)
            # JSON pretty-print style may differ between JS and Python, not its native result.
            if actual.get('results') != recorded['results'] or actual.get('kind') != recorded['kind']:
                raise ValueError('independent native UI replay differs; CSS is not native axis proof')
            replayed.add(key)
        else:
            if font['original_path'] not in {item['path'] for item in matches}:
                raise ValueError('resolved font origin differs from runtime inventory')
            if font['resolution'] == 'inventory_unique':
                if len({item['path'] for item in matches}) != 1 or font['coretext_probe'] is not None:
                    raise ValueError('ambiguous font path cannot be inferred as unique')
            elif font['resolution'] == 'coretext_exact':
                probe = load(_client_artifact(root, pack, font['coretext_probe']))
                environment = load(_client_artifact(root, pack, generation['font_probe_environment']))
                selected = environment.get('sdk_selection') == 'macosx'
                commands = ([['/usr/bin/xcrun', '--sdk', 'macosx', 'swiftc', '--version'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-path'], ['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version'], ['/usr/bin/sw_vers']] if selected else
                            [['/usr/bin/swift', '--version'], ['/usr/bin/xcrun', '--show-sdk-path'], ['/usr/bin/xcrun', '--show-sdk-version'], ['/usr/bin/sw_vers']])
                if (not isinstance(environment.get('commands'), list)
                        or [r.get('command') for r in environment['commands']] != commands
                        or any(type(r.get('exit_code')) is not int or r['exit_code'] != 0 or not isinstance(r.get('stdout'), str) or not r['stdout'].strip() for r in environment['commands'])):
                    raise ValueError('actual CoreText SDK/system identity missing')
                expected_command = (['/usr/bin/xcrun', '--sdk', 'macosx', 'swift', '-sdk', environment['commands'][1]['stdout'].strip(), '-e', CORETEXT_FONT_PROBE, font['postscript_name']] if selected else ['/usr/bin/swift', '-e', CORETEXT_FONT_PROBE, font['postscript_name']])
                if (probe.get('command') != expected_command
                        or type(probe.get('exit_code')) is not int or probe['exit_code'] != 0 or probe.get('error') is not None):
                    raise ValueError('actual CoreText font probe required')
                located = json.loads(probe['stdout'], object_pairs_hook=_unique)
                if (located.get('requested') != font['postscript_name'] or located.get('resolved') != font['postscript_name']
                        or located.get('url') != font['original_path']):
                    raise ValueError('CoreText fallback or path conflict cannot attest requested font')
            else: raise ValueError('unknown font resolution authority')
    return {'status': 'measured', 'public_ui_probes_independently_replayed': len(replayed),
            'native_verified': False, 'product_alignment_approved': False}



def validate_c1_layout_geometry(geometry, surface, layout):
    _client_shape(geometry, ('viewport', 'surface', 'sidebar', 'reading', 'preview', 'content_scroll', 'preview_reachability'),
                  'complete approved C1 geometry measurements required')
    viewport = {k:surface['viewport'][k] for k in ('width','height')}
    if geometry['viewport'] != viewport or geometry['surface'] != {'x':0,'y':0,**viewport}:
        raise ValueError('actual C1 viewport/surface differs from approved container')
    contract = layout['contract']['roles'][surface['role']][surface['variant']]
    for key in ('sidebar','reading','preview'):
        box = geometry[key]
        if box is None and key != 'sidebar': continue
        _client_shape(box, ('x','y','width','height'), 'actual C1 box required')
        if any(type(v) not in (int,float) or not math.isfinite(v) for v in box.values()):
            raise ValueError('finite C1 geometry required')
    if geometry['sidebar']['width'] != contract['sidebar_width']:
        raise ValueError('actual C1 sidebar differs from approved width')
    preview = geometry['preview']; reading = geometry['reading']
    if surface['role'] == 'main' and preview is not None:
        if preview['width'] != 282: raise ValueError('actual C1 preview differs from approved width')
        if surface['variant'] == 'narrow' and (reading is None or preview['y'] < reading['y'] + reading['height']):
            raise ValueError('actual C1 narrow preview is not after reading')
    if not isinstance(geometry['content_scroll'], dict): raise ValueError('actual C1 scroll area required')
    if preview is not None:
        reachability = geometry['preview_reachability']
        if (not isinstance(reachability, dict) or reachability.get('fully_visible') is not True
                or reachability.get('restored') is not True
                or reachability.get('visible_width') != preview['width']
                or reachability.get('visible_height') != preview['height']):
            raise ValueError('C1 preview is not reachable through the real content scroller')
    elif geometry['preview_reachability'] is not None:
        raise ValueError('C1 absent preview cannot claim scroll reachability')


def replay_c1_layout(root, pack, generation, request, row, measurements):
    """Execute the frozen recipe and approved transform again in locked Chromium.

    This fresh process rebuilds from the original approved source bytes, never
    from candidate computed CSS, candidate HTML or a desktop screenshot crop.
    Its raw failed output is kept; it grants no reference/product approval.
    """
    import uuid
    from reading_alignment_preflight import png_pixels
    from current_design_render_qualification import compare_client_render_facts
    destination = safe_path(root, '.build') / 'client-layout-replays' / uuid.uuid4().hex
    destination.mkdir(parents=True, exist_ok=False)
    repeated_request = copy.deepcopy(request); repeated_request['execution_nonce'] = uuid.uuid4().hex
    request_file = destination/'request.json'
    request_file.write_text(json.dumps(repeated_request, ensure_ascii=False, indent=2)+'\n')
    command = ['node', str(root/CLIENT_RUNNER), '--request', str(request_file), '--output', str(destination)]
    started = datetime.now().astimezone().isoformat()
    try:
        process = subprocess.run(command, cwd=root, capture_output=True, text=True, timeout=300)
        execution = {'command':command,'started_at':started,'finished_at':datetime.now().astimezone().isoformat(),
                     'exit_code':process.returncode,'stdout':process.stdout,'stderr':process.stderr}
    except (OSError,subprocess.SubprocessError) as error:
        execution = {'command':command,'started_at':started,'finished_at':datetime.now().astimezone().isoformat(),
                     'exit_code':None,'error':str(error)}
    (destination/'actual-command.json').write_text(json.dumps(execution,ensure_ascii=False,indent=2)+'\n')
    if execution['exit_code'] != 0:
        raise ValueError('independent C1 source/layout replay failed; raw run retained at '+str(destination))
    repeated = load(destination/'verification.json')
    if (repeated.get('schema_version') != 3 or repeated.get('status') != 'passed'
            or repeated.get('approved_layout') != request['approved_layout']
            or repeated.get('generator') != request['generator'] or repeated.get('browser') != request['browser']
            or repeated.get('sources_before') != generation['sources_before']
            or repeated.get('sources_after') != generation['sources_after']
            or repeated.get('execution_nonce') != repeated_request['execution_nonce']
            or any(repeated.get(key) != [] for key in ('source_drift','page_errors','console_errors','request_errors','external_requests','fatal_errors'))):
        raise ValueError('independent C1 source/layout replay identity differs')
    actual = next(item for item in repeated['results'] if item['state_id'] == row['state_id'])
    if actual.get('status') != 'passed' or actual.get('errors') != [] or actual.get('effective_recipe') != row['effective_recipe']:
        raise ValueError('independent C1 recipe or result differs')
    actual_measurement = load(_client_artifact(root,destination,actual['measurements']))
    def normalized_document(document, source_url):
        source = urlsplit(source_url); origin = source.scheme + '://' + source.netloc
        return _ClientTree(document.replace(origin, '<controlled-client-origin>')).root
    raw_original = load(_client_artifact(root,pack,row['original_dom']))
    raw_derived = load(_client_artifact(root,pack,row['client_dom']))
    fresh_original = load(_client_artifact(root,destination,actual['original_dom']))
    fresh_derived = load(_client_artifact(root,destination,actual['client_dom']))
    for label, raw_document, fresh_document in [('original',raw_original,fresh_original),('derived',raw_derived,fresh_derived)]:
        if (normalized_document(raw_document['document_html'],raw_original['url'])
                != normalized_document(fresh_document['document_html'],fresh_original['url'])):
            raise ValueError('independent C1 '+label+' DOM differs from controlled approved-source replay')
    validate_c1_layout_geometry(actual_measurement.get('approved_layout_geometry'), request['client_surface'],request['approved_layout'])
    comparison = compare_client_render_facts(measurements['facts'],actual_measurement['facts'],request['client_surface'])
    if comparison['differences'] or actual_measurement['approved_layout_geometry'] != measurements['approved_layout_geometry']:
        raise ValueError('independent C1 render facts or client geometry differ: '+str(comparison['differences']))
    expected_png = _client_artifact(root,pack,row['client_png'])
    actual_png = _client_artifact(root,destination,actual['client_png'])
    if png_pixels(expected_png.read_bytes()) != png_pixels(actual_png.read_bytes()):
        raise ValueError('independent C1 actual client pixels differ (zero threshold retained)')
    result = {'status':'recomputed','capture_scope':'whole_client','approved_layout_id':request['approved_layout']['contract']['id'],
              'command':command,'generation':{'path':str((destination/'verification.json').relative_to(root)),
                                            'sha256':digest(destination/'verification.json')},
              'geometry_equal':True,'effective_render_equal':True,'actual_client_pixels_equal':True,
              'original_dom_equal':True,'derived_dom_equal':True,
              'reference_approved':False,'native_verified':False,'product_alignment_approved':False,'completion_allowed':False}
    (destination/'layout-replay-result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    return result


def validate_client_capture(root, pack_root, generation_ref, state_id, *, context=None):
    """Read exact fresh source-run artifacts. A good reference is not a product pass."""
    root = Path(root).resolve()
    context = authoritative_context(root) if context is None else context
    pack = safe_path(root, pack_root)
    if not pack.is_dir() or not pack.is_relative_to(root / '.build'):
        raise ValueError('immutable client reference pack required')
    generation = load(_hashed(pack, generation_ref))
    c1 = generation.get('schema_version') == 3
    if (type(generation.get('schema_version')) is not int or generation.get('schema_version') not in (2, 3)
            or generation.get('kind') != ('current-client-reference-generation/v3' if c1 else 'current-client-reference-generation/v2')
            or generation.get('collection_complete') is not True
            or generation.get('prototype_reference_only') is not True
            or generation.get('native_verified') is not False
            or generation.get('product_alignment_approved') is not False):
        raise ValueError('whole client generation schema/capability differs')
    request = load(_hashed(pack, generation['request']))
    request_keys = ('schema_version', 'kind', 'target_id', 'composition_id', 'design_closure_sha256',
                    'design_sources', 'runtime_assets', 'execution_nonce', 'authors', 'clock_iso',
                    'generator', 'state_descriptors', 'client_surface', 'browser', *(['approved_layout'] if c1 else []))
    _client_shape(request, request_keys, 'exact client request schema required')
    if type(request['schema_version']) is not int or request['schema_version'] != (3 if c1 else 2) or request['kind'] != ('current-client-reference-request/v3' if c1 else 'current-client-reference-request/v2'):
        raise ValueError('target request cannot grant whole client scope')
    surface = request['client_surface']
    layout = client_c1_authority(root) if c1 else None
    if c1 and (request.get('approved_layout') != layout or generation.get('approved_layout') != layout
               or layout['parent_design_sources'] != context['design_sources']
               or layout['parent_design_closure_sha256'] != context['design_closure_sha256']):
        raise ValueError('C1 source approval or parent 61-source closure differs')
    if surface != (client_c1_surface(root, surface['role'], surface['variant']) if c1 else client_surface(surface['role'], surface['variant'])):
        raise ValueError('client surface contract differs from code-owned context')
    # The explicit context seam is only used by module-owned mutation fixtures;
    # production assessment builds this identity from the installed repository.
    browser = context.get('client_browser') or client_browser_identity(root)
    _client_shape(generation.get('browser'), ('version', 'executable', 'sha256', 'playwright'),
                  'actual managed client browser identity missing')
    if request['browser'] != browser or generation['browser'] != browser:
        raise ValueError('actual managed client browser version/executable/hash/PW differs')
    executable = Path(browser['executable'])
    if (not executable.is_absolute() or not executable.is_file() or digest(executable) != browser['sha256']
            or not re.fullmatch(r'\d+\.\d+\.\d+\.\d+', browser['version'])
            or not re.fullmatch(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', browser['playwright'])):
        raise ValueError('actual managed client browser binary/version unavailable')
    for key in ('target_id', 'composition_id', 'design_closure_sha256', 'design_sources', 'runtime_assets'):
        if request[key] != context[key] or generation.get(key) != context[key]:
            raise ValueError('client current source authority differs: ' + key)
    expected_sources = {**context['design_sources'], **context['runtime_assets']}
    if c1:
        expected_sources.update({p:r['sha256'] for p,r in layout['sources'].items()})
        expected_sources[layout['approval']['path']] = layout['approval']['sha256']
    for path, sha in expected_sources.items():
        if digest(safe_path(root, path)) != sha: raise ValueError('client original source bytes drift')
    if (generation.get('sources_before') != expected_sources or generation.get('sources_after') != expected_sources
            or any(generation.get(k) != [] for k in ('page_errors', 'console_errors', 'request_errors',
                                                    'external_requests', 'fatal_errors', 'source_drift'))):
        raise ValueError('client capture source epoch/runtime violation')
    generator = {'path': CLIENT_RUNNER, 'sha256': digest(safe_path(root, CLIENT_RUNNER))}
    if request['generator'] != generator or generation.get('generator') != generator:
        raise ValueError('client source generator identity differs')
    if (not _actors(request['authors']) or generation.get('authors') != request['authors']
            or not isinstance(request['execution_nonce'], str) or not NONCE.fullmatch(request['execution_nonce'])
            or generation.get('execution_nonce') != request['execution_nonce']):
        raise ValueError('actual client authors/nonce required')
    _date(request['clock_iso']); start, end = _date(generation['started_at']), _date(generation['finished_at'])
    if end < start: raise ValueError('client capture interval reversed')
    command = generation['command']; workspace = Path(generation['workspace_root'])
    origin = Path(generation['original_output']); original_request = Path(generation['original_request'])
    if (not workspace.is_absolute() or not origin.is_relative_to(workspace / '.build')
            or original_request != origin / 'request.json' or len(command) != 6
            or Path(command[0]).name != 'node' or command[1:] != [str(workspace / CLIENT_RUNNER), '--request', str(original_request), '--output', str(origin)]):
        raise ValueError('fixed whole client capture command differs')
    requested = {}
    for item in request['state_descriptors']:
        _client_shape(item, ('state_id', 'descriptor_json', 'descriptor_sha256'), 'client descriptor schema differs')
        sid = item['state_id']; value = load_descriptor(item)
        if sid in requested or value != context['descriptors'].get(sid) or value['target']['container'] != surface['role']:
            raise ValueError('exact unique client state recipe required')
        requested[sid] = item
    rows = generation['results']
    if (not requested or not isinstance(rows, list) or len(rows) != len(requested)
            or {r['state_id'] for r in rows} != set(requested) or state_id not in requested):
        raise ValueError('exact client capture state closure required')
    expected_status = 'failed' if any(r.get('status') == 'failed' for r in rows) else 'blocked' if any(r.get('status') == 'blocked' for r in rows) else 'passed'
    if generation.get('status') != expected_status:
        raise ValueError('whole client generation status contradicts actual rows')
    row = next(r for r in rows if r['state_id'] == state_id)
    if row.get('status') != 'passed' or row.get('errors') != [] or row.get('capture_scope') != 'whole_client':
        raise ValueError('client capture failed or lacks whole client scope')
    item = requested[state_id]; value = load_descriptor(item)
    if row.get('descriptor_sha256') != item['descriptor_sha256']:
        raise ValueError('client result descriptor differs')
    actions = row.get('actions'); recipe = client_c1_recipe(value, layout) if c1 else value['design_recipe']
    if c1 and row.get('effective_recipe') != recipe: raise ValueError('C1 effective required-query recipe differs')
    if (not isinstance(actions, list) or len(actions) != len(recipe)
            or any(a != {'index': i, 'phase': 'recipe', 'step': step, 'status': 'passed', 'actual': a.get('actual')}
                   or not _action_actual(step, a.get('actual')) for i, (a, step) in enumerate(zip(actions, recipe)))):
        raise ValueError('actual whole client recipe transcript differs')
    raw = load(_client_artifact(root, pack, row['original_dom']))
    derived = load(_client_artifact(root, pack, row['client_dom']))
    if (raw.get('surface_selector') != surface['selector'] or derived.get('surface_selector') != surface['selector']
            or not isinstance(raw.get('document_html'), str) or not raw['document_html'].strip()
            or not isinstance(raw.get('surface_html'), str) or not raw['surface_html'].strip()
            or derived.get('original_surface_html') != raw['surface_html']
            or derived.get('surface_html') != raw['surface_html']
            or not isinstance(derived.get('document_html'), str) or not derived['document_html'].strip()
            or derived.get('projection') != surface['projection']):
        raise ValueError('original and derived client DOM provenance missing')
    css = safe_path(root, layout['contract']['styles']).read_text() if c1 else client_projection_css(surface['selector'])
    original_html, derived_html = _ClientHtml(raw['document_html']), _ClientHtml(derived['document_html'])
    if (derived_html.executable or derived_html.links != original_html.links
            or derived_html.styles != original_html.styles + [css]):
        raise ValueError('derived client changed original styles or executable boundary')
    validate_client_projection(raw, derived, surface, approved_layout={'css': css} if c1 else None)
    navigation = _final_navigation(recipe); url = urlsplit(raw['url'])
    if (url.scheme != 'http' or url.hostname != '127.0.0.1'
            or unquote(url.path) != '/' + navigation['path']
            or parse_qs(url.query) != {k: [v] for k, v in navigation['query'].items()}):
        raise ValueError('original client DOM route differs')
    measurements = load(_client_artifact(root, pack, row['measurements']))
    if measurements.get('client_surface') != surface or measurements.get('projection') != surface['projection']:
        raise ValueError('client measurement projection identity differs')
    validate_client_facts(measurements['facts'], surface)
    validate_client_font_evidence(root, pack, generation, measurements['facts'])
    original_box = measurements['original_surface_box']
    if (any(type(original_box.get(k)) not in (int, float) or not math.isfinite(original_box[k]) for k in ('x', 'y', 'width', 'height'))
            or (not c1 and (original_box['width'] != surface['viewport']['width'] or original_box['height'] != surface['viewport']['height']))):
        raise ValueError('original client dimensions differ; resizing is forbidden')
    from reading_alignment_preflight import png_pixels
    for key, size in (('client_png', tuple(surface['viewport'][k] for k in ('width', 'height'))),
                      ('original_png', tuple(measurements['original_viewport'][k] for k in ('width', 'height')))):
        path = _client_artifact(root, pack, row[key])
        if png_pixels(path.read_bytes(), decode_pixels=False)[:2] != size:
            raise ValueError('actual client PNG dimensions differ')
    if c1:
        validate_c1_layout_geometry(measurements.get('approved_layout_geometry'), surface, layout)
        transform = row.get('approved_transform')
        expected_transform = {'id': surface['projection'], 'approval': layout['approval'], 'sources': layout['sources'],
            'bootstrap_sha256': layout['sources'][layout['contract']['bootstrap']]['sha256'],
            'query': {k:v[0] for k,v in parse_qs(url.query).items()},
            'root_attributes': {'data-client-candidate': 'c1', 'data-client-role': surface['role'], 'data-client-settings': 'native'}}
        if transform != expected_transform: raise ValueError('C1 bootstrap/approval transformation differs')
        replay = replay_c1_layout(root, pack, generation, request, row, measurements)
    return {'state_id': state_id, 'capture_scope': 'whole_client', 'surface': surface,
            'facts': measurements['facts'], 'row': row, 'generation': generation,
            'authors': request['authors'], 'pack_root': pack_root,
            'native_verified': False, 'product_alignment_approved': False,
            **({'approved_layout': layout, 'independent_layout_replay': replay} if c1 else {})}


def assess_client_catalogue(root, catalogue_path, review_path, required=None, *, context=None):
    """V2 per-entry packs coexist with all legacy v1 references and reviews."""
    root = Path(root).resolve(); errors, gaps, approved = [], [], set()
    result = {'schema_version': 2, 'kind': 'current-client-reference-assessment/v2',
              'status': 'failed', 'errors': errors, 'gaps': gaps, 'approved_state_ids': [],
              'capture_scope': 'whole_client', 'native_verified': False, 'product_alignment_approved': False}
    try:
        context = authoritative_context(root) if context is None else context
        catalogue_file = safe_path(root, catalogue_path); catalogue = load(catalogue_file)
        _client_shape(catalogue, ('schema_version', 'kind', 'target_id', 'composition_id',
                                 'design_closure_sha256', 'entries'), 'exact v2 client catalogue required')
        if type(catalogue['schema_version']) is not int or catalogue['schema_version'] != 2 or catalogue['kind'] != 'current-client-reference-catalogue/v2':
            raise ValueError('target catalogue cannot grant whole client scope')
        for key in ('target_id', 'composition_id', 'design_closure_sha256'):
            if catalogue[key] != context[key]: raise ValueError('client catalogue authority differs')
        entries = catalogue['entries']
        if not isinstance(entries, list) or not entries or len({e['state_id'] for e in entries}) != len(entries):
            raise ValueError('unique client catalogue entries required')
        authors, validated = set(), {}
        for entry in entries:
            _client_shape(entry, ('state_id', 'pack_root', 'generation', 'result_sha256'), 'per-entry immutable client origin required')
            capture = validate_client_capture(root, entry['pack_root'], entry['generation'], entry['state_id'], context=context)
            if entry['result_sha256'] != identity(capture['row']): raise ValueError('client result origin digest differs')
            authors.update(capture['authors']); validated[entry['state_id']] = capture
        review_file = safe_path(root, review_path)
        if not review_file.is_file(): gaps.append('independent whole client reference review absent')
        else:
            review = load(review_file)
            if review.get('status') == 'pending_review': gaps.append('independent whole client reference review pending')
            else:
                if (review.get('schema_version') != 2 or type(review.get('schema_version')) is not int
                        or review.get('kind') != 'independent-current-client-reference-review/v2'
                        or review.get('status') != 'approved' or review.get('independent_non_author_review') is not True
                        or not _actor(review.get('reviewer_id')) or review['reviewer_id'] in authors
                        or review.get('author_ids') != sorted(authors)
                        or review.get('catalogue') != {'path': catalogue_path, 'sha256': digest(catalogue_file)}
                        or any(review.get(k) is not False for k in ('source_approval_extended', 'product_alignment_approved', 'native_verified'))):
                    raise ValueError('independent client review authority differs')
                for key in ('target_id', 'composition_id', 'design_closure_sha256'):
                    if review.get(key) != context[key]: raise ValueError('client review source identity differs')
                reviewed = review.get('reviewed_entries')
                if (not isinstance(reviewed, dict) or not reviewed or not set(reviewed) <= set(validated)
                        or any(sha != identity(next(e for e in entries if e['state_id'] == sid)) for sid, sha in reviewed.items())
                        or any(_date(review['reviewed_at']) < _date(validated[sid]['generation']['finished_at']) for sid in reviewed)):
                    raise ValueError('client review entry/time differs')
                approved = set(reviewed)
        required = set(validated) if required is None else set(required)
        if not required <= context['known_states']: raise ValueError('unknown client reference requirement')
        if required - approved: gaps.append('independently reviewed whole client states missing: ' + ','.join(sorted(required - approved)))
        result.update(status='blocked' if gaps else 'passed', approved_state_ids=sorted(approved))
    except (OSError, ValueError, TypeError, KeyError, AttributeError, IndexError, StopIteration, struct.error, zlib.error) as error:
        errors.append('whole client reference invalid: ' + str(error))
    return result
