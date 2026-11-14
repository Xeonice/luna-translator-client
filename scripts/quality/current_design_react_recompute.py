"""Independently decode original pixels and parse DOM; never trust pass labels."""
from html.parser import HTMLParser
import hashlib
import json
from pathlib import Path
import struct
import zipfile
import zlib

from current_design_react_inspect import artifact, _file
from current_design_react_adapter import STATE_IDS
from current_reference_catalogue import load, canonical
from gate_core import safe_path, digest
from reading_alignment_preflight import png_pixels, enclosing_png_size


class DocumentFacts(HTMLParser):
    """Read retained markup, including hidden retired controls, without a browser."""
    def __init__(self, html):
        super().__init__(convert_charrefs=True)
        self.nodes = []; self.stack = []; self.sources = []; self.paragraphs = []
        self.feed(html); self.close()

    def handle_starttag(self, tag, attributes):
        node = {'tag': tag, 'attrs': dict(attributes), 'text': '', 'without_small': '', 'source': False,
                'index': len(self.nodes), 'ancestors': [n['index'] for n in self.stack]}
        node['source'] = node['attrs'].get('data-testid') == 'source-text'
        self.nodes.append(node)
        if tag not in {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr'}:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if self.stack and self.stack[-1]['tag'] == tag: self.handle_endtag(tag)

    def handle_data(self, data):
        if any(n['tag'] in {'rt', 'rp', 'script', 'style'} for n in self.stack): return
        for node in self.stack:
            node['text'] += data
            if not any(n['tag'] == 'small' for n in self.stack): node['without_small'] += data

    def handle_endtag(self, tag):
        indexes = [i for i, n in enumerate(self.stack) if n['tag'] == tag]
        if indexes: del self.stack[indexes[-1]:]

    def source_texts(self): return [n['text'] for n in self.nodes if n['source']]
    def count(self, **attrs): return sum(all(n['attrs'].get(k) == v for k, v in attrs.items()) for n in self.nodes)

    def switch_value(self, label):
        nodes = [n for n in self.nodes if n['attrs'].get('aria-label') == label and n['attrs'].get('role') in ('switch', 'checkbox')]
        if len(nodes) != 1: raise ValueError('exact normal-state switch missing or duplicated: ' + label)
        attrs = nodes[0]['attrs']
        if 'aria-checked' in attrs:
            if attrs['aria-checked'] not in ('true', 'false'): raise ValueError('normal-state switch has invalid checked state')
            return attrs['aria-checked'] == 'true'
        return 'checked' in attrs

    def body_rows(self, *, design=False):
        records = [n for n in self.nodes if ('data-flat-history-record' in n['attrs'] if design
                   else n['attrs'].get('data-record-kind') == 'body' and 'data-record-sequence' in n['attrs'])]
        rows = []
        for record in records:
            children = [n for n in self.nodes if record['index'] in n['ancestors']]
            paragraphs = [n for n in children if n['tag'] == 'p']
            if design:
                sequence = next((n['text'].strip().removeprefix('#') for n in children if 'rw-sequence' in n['attrs'].get('class', '').split()), None)
                translation = next((n['text'].strip() for n in children if 'rw-history-translation' in n['attrs'].get('class', '').split()), None)
            else:
                sequence = record['attrs']['data-record-sequence']
                translation = paragraphs[1]['without_small'].strip() if len(paragraphs) > 1 else None
            rows.append({'sequence': sequence, 'source': paragraphs[0]['text'].strip() if paragraphs else None, 'translation': translation})
        return rows


def pixel_facts(reference, candidate, diff=None):
    aw, ah, a = png_pixels(reference); bw, bh, b = png_pixels(candidate)
    w, h = max(aw, bw), max(ah, bh)
    if w > 4096 or h > 4096: raise ValueError('bounded original PNG dimensions required')
    different = delta = 0; expected_diff = bytearray(w * h * 4) if diff is not None else None
    for y in range(h):
        for x in range(w):
            av = a[(y * aw + x) * 4:(y * aw + x + 1) * 4] if y < ah and x < aw else b'\0\0\0\0'
            bv = b[(y * bw + x) * 4:(y * bw + x + 1) * 4] if y < bh and x < bw else b'\0\0\0\0'
            changed = av != bv
            different += changed; delta += sum(abs(left - right) for left, right in zip(av, bv))
            if expected_diff is not None: expected_diff[(y * w + x) * 4:(y * w + x + 1) * 4] = b'\xff\0\0\xff' if changed else av
    if diff is not None:
        dw, dh, pixels = png_pixels(diff)
        if (dw, dh) != (w, h) or pixels != expected_diff: raise ValueError('actual diagnostic diff PNG does not equal original RGBA comparison')
    return {'reference_size': [aw, ah], 'candidate_size': [bw, bh], 'comparison_canvas': [w, h],
            'different_pixels': different, 'absolute_channel_delta': delta,
            'status': 'passed' if different == 0 and (aw, ah) == (bw, bh) else 'failed'}


def recompute(root, receipt, context):
    errors = []
    try:
        root = Path(root); report_path = _file(root, receipt['report']); out = report_path.parent; r = load(report_path)
        if r['execution_nonce'] != context['execution_nonce'] or r['layer'] != 'real_react': raise ValueError('actual raw layer/nonce differs')
        request = load(artifact(out, r['request'])); package = safe_path(root, request['artifact_path'])
        if digest(package) != context['artifact_sha256']: raise ValueError('exact package hash differs')
        from package_identity import IDENTITY_ENTRY, EXECUTABLE_ENTRY
        with zipfile.ZipFile(package) as archive:
            identity = json.loads(archive.read(IDENTITY_ENTRY))
            seal = json.loads(archive.read('Luna.app/Contents/Resources/luna-source-identity.json'))
            if (identity['source_tree_sha256'] != receipt['source_tree_sha256'] or seal['source_tree_sha256'] != receipt['source_tree_sha256']
                    or hashlib.sha256(archive.read(EXECUTABLE_ENTRY)).hexdigest() != identity['executable_sha256']):
                raise ValueError('real sealed final package source/executable differs')
        catalogue = load(root / 'docs/development/current-reference-catalogue.json'); pack = safe_path(root, catalogue['pack_root'])
        refs = {e['state_id']: e for e in catalogue['entries']}
        rows = r['results']
        if len(rows) != len(STATE_IDS) or {v['state_id'] for v in rows} != STATE_IDS: raise ValueError('exact four original comparisons required')
        for row in rows:
            sid = row['state_id']; comparison = row['pixel_comparison']
            if (comparison['algorithm'] != 'strict_rgba_all_channels' or type(comparison['tolerance']) is not int or comparison['tolerance'] != 0
                    or type(comparison['max_different_pixels']) is not int or comparison['max_different_pixels'] != 0
                    or comparison['originals_unchanged'] is not True
                    or comparison['dimension_padding'] != 'diagnostic canvas only, never image resize'):
                raise ValueError('strict original pixel policy differs')
            a = artifact(out, row['reference_png']); b = artifact(out, row['target_png'])
            if digest(a) != refs[sid]['target_png']['sha256']: raise ValueError('independent original reference substituted')
            actual = pixel_facts(a.read_bytes(), b.read_bytes(), artifact(out, comparison['diff']).read_bytes())
            if any(type(comparison[k]) is not type(v) or comparison[k] != v for k, v in actual.items()):
                raise ValueError('reported RGBA comparison differs from independent original decoding')
            if actual['status'] != 'passed': errors.append(sid + ': original pixels/dimensions differ')
            geometry = load(artifact(out, row['geometry'])); source_geometry = load(artifact(pack, refs[sid]['geometry']))
            viewport = geometry['viewport']; box = geometry['target_box']
            if (viewport != source_geometry['viewport'] or viewport['deviceScaleFactor'] != 1
                    or png_pixels(artifact(out, row['full_png']).read_bytes(), decode_pixels=False)[:2] != (viewport['width'], viewport['height'])
                    or actual['candidate_size'] != enclosing_png_size(box, geometry['scroll'])):
                raise ValueError('same viewport/original target geometry differs')
            if geometry['clip']['within_viewport'] is not True: errors.append(sid + ': original target is clipped')
            dom = load(artifact(out, row['production_dom'])); inputs = load(artifact(out, row['inputs']))
            target = DocumentFacts(dom['target_html']); document = DocumentFacts(dom['document_html'])
            source = source_geometry['observations_after']['designState']; raw = source['current']
            for key, source_key in (('segmentationEnabled', 'segmentation'), ('showRuby', 'furigana'), ('showPos', 'pos')):
                if type(inputs['preferences'][key]) is not bool or inputs['preferences'][key] != source['settings']['general'][source_key]:
                    raise ValueError('actual controlled preferences differ from independent source')
            if sid == 'all-page-main-session-normal':
                if target.source_texts() != [raw['source']]: errors.append(sid + ': retained actual source DOM differs')
                if not any(n['without_small'].strip() == raw['translation'] for n in target.nodes): errors.append(sid + ': retained actual translation DOM differs')
            if sid.endswith('readingpanel-dataset-body-retired-surface-absent'):
                generation = load(safe_path(pack, catalogue['generation']['path']))
                source_row = next(v for v in generation['results'] if v['state_id'] == sid)
                source_dom = load(artifact(pack, source_row['source_dom']))
                expected_rows = DocumentFacts(source_dom['target_html']).body_rows(design=True)
                if not expected_rows or target.body_rows() != expected_rows:
                    errors.append(sid + ': retained complete body history DOM differs from original reference')
                if inputs['actual_visible']['body_rows'] != target.body_rows(): raise ValueError('body observation contradicts retained production DOM')
                if document.count(role='tablist', **{'aria-label': '当前会话内容'}) != 0: errors.append(sid + ': retired session tabs remain in actual DOM')
                details = [n for n in target.nodes if n['tag'] == 'details' and n['attrs'].get('data-flat-session-section') == 'reading']
                if len(details) != 1 or 'open' not in details[0]['attrs']: errors.append(sid + ': actual body disclosure is not open')
            if sid.endswith('modelconfigscreen-tab-connections-retired-surface-absent'):
                if document.count(**{'aria-label': '模型配置实体'}) != 0: errors.append(sid + ': retired model entity tabs remain in actual DOM')
            if sid == 'all-page-settings-general-normal':
                mapped = {'general.segmentation': ('可点击日语分词', 'segmentation'),
                          'general.furigana': ('显示日语注音', 'furigana'),
                          'general.pos': ('显示词性标记', 'pos')}
                for _field, (label, key) in mapped.items():
                    if document.switch_value(label) != source['settings']['general'][key]:
                        errors.append(sid + ': actual mapped normal-state switch differs: ' + key)
                # Missing approved source controls remain a real product gap.
                for control in inputs['visible_source_dom']['general_controls']:
                    if control['field'] in mapped: continue
                    if not any(n['attrs'].get('aria-label') == control['label'] and n['attrs'].get('role') in ('switch', 'checkbox') for n in document.nodes):
                        errors.append(sid + ': approved general control is absent: ' + control['field'])
            # This launches a new offline browser on pinned original DOM/assets;
            # producer style JSON or a nominal eligible label is insufficient.
            from current_design_render_qualification import inspect_qualification
            errors.extend(inspect_qualification(root, report_path, row, fresh=True))
            if row['same_data_eligibility']['status'] != 'eligible': errors.append(sid + ': no eligible same-state mapping')
    except (OSError, ValueError, TypeError, KeyError, AttributeError, IndexError, zipfile.BadZipFile, zlib.error, struct.error) as error:
        errors.append('current React independent recomputation invalid: ' + str(error))
    return errors
