#!/usr/bin/env python3
"""Check and render model-service design documents; never execute the product."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import html
from html.parser import HTMLParser
import importlib
import json
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[2]
CATALOG = 'docs/architecture/model-service-registry-design.json'
PRIMARY = 'docs/architecture/japanese-study-and-llm.md'
DOCS = (
    ('technical-design', '语言学习与模型服务技术设计', PRIMARY),
    ('model-configuration', '模型配置产品需求', 'docs/product/model-configuration-prd.md'),
    ('engineering-baseline', '首版研发基线', 'docs/architecture/first-product-engineering-baseline.md'),
    ('database-design', '数据库设计与迁移边界', 'docs/architecture/database-design.md'),
    ('product-index', '产品文档入口', 'docs/product/README.md'),
    ('engineering-gates', '工程门禁与验收边界', 'docs/development/engineering-gates.md'),
)
PURPOSES = frozenset({'translation', 'grammar', 'context_word'})
EXISTING_PROTOCOLS = frozenset({'openai_compatible', 'anthropic', 'azure_translator'})
MARKERS = {'protocols': ('<!-- model-service-protocols:start -->', '<!-- model-service-protocols:end -->'),
           'presets': ('<!-- model-service-presets:start -->', '<!-- model-service-presets:end -->')}


def _safe(root: Path, relative: str) -> Path:
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute() or '..' in Path(relative).parts:
        raise ValueError(f'Unsafe source path: {relative!r}')
    target = root / relative
    for parent in (target, *target.parents):
        if parent == root:
            break
        if parent.is_symlink():
            raise ValueError(f'Source symlink is forbidden: {relative}')
    if not target.resolve().is_relative_to(root.resolve()) or not target.is_file():
        raise ValueError(f'Missing source document: {relative}')
    return target


def _parse_catalog(raw: bytes) -> dict:
    def unique(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError(f'Duplicate catalog JSON key: {key}')
            value[key] = item
        return value
    document = json.loads(raw.decode('utf-8'), object_pairs_hook=unique,
                          parse_constant=lambda value: (_ for _ in ()).throw(ValueError(f'Non-JSON value: {value}')))
    if not isinstance(document, dict):
        raise ValueError('Catalog must be a JSON object')
    return document


def load_catalog(root: Path) -> dict:
    return _parse_catalog(_safe(Path(root).resolve(), CATALOG).read_bytes())


def _cell(value) -> str:
    return html.escape(str(value), quote=False).replace('\\', '\\\\').replace('|', '\\|').replace('\r', '').replace('\n', '<br>')


def _table(headers, rows) -> str:
    return '\n'.join(['| ' + ' | '.join(headers) + ' |', '| ' + ' | '.join('---' for _ in headers) + ' |',
                      *['| ' + ' | '.join(_cell(value) for value in row) + ' |' for row in rows]])


def render_protocol_table(catalog: dict) -> str:
    """Canonical Markdown only; does not write or approve the source document."""
    return _table(['协议 ID', '协议', '契约版本', '类型', '认证', '模型身份', '响应', '用途', '源码状态', '适配／实服验收'],
                  [(p['id'], p['label'], p['contract_revision'], p['family'], p['auth_description'], p['model_identity'],
                    p['wire_output'], ', '.join(p['allowed_purposes']), p['source_state'],
                    p['adapter_acceptance'] + ' / ' + p['live_acceptance']) for p in catalog['protocols']])


def render_preset_table(catalog: dict) -> str:
    """Canonical versioned route matrix, preserving separate TokenHub routes."""
    return _table(['预设 ID', '名称', '路由', 'Provider ID', '协议／契约版本', '兼容配置／版本', '预设版本', '用途', '实现／验收'],
                  [(p['id'], p['label'], p['direction_id'], p['provider_id'],
                    f"{p['protocol_id']}@{p['adapter_contract_revision']}",
                    f"{p['compatibility_profile_id']}@{p['profile_revision']}", p['preset_revision'],
                    ', '.join(p['allowed_purposes']), p['implementation_status'] + ' / ' + p['acceptance_status'])
                   for p in catalog['presets']])


def validate_catalog(root: Path, catalog: dict) -> list[str]:
    if not isinstance(catalog, dict):
        return ['Catalog must be a JSON object']
    errors = []
    text = lambda value: isinstance(value, str) and bool(value.strip()) and not any(ord(c) < 32 for c in value)
    def strings(value):
        return isinstance(value, list) and bool(value) and all(text(item) for item in value) and len(value) == len(set(value))
    def positive(value):
        return type(value) is int and value > 0
    def rows(name):
        value = catalog.get(name)
        if not isinstance(value, list) or not value or any(not isinstance(item, dict) for item in value):
            errors.append(f'{name}: nonempty object list required')
            return []
        ids = [row.get('id') for row in value]
        if not all(text(item) for item in ids) or len(ids) != len(set(str(item) for item in ids)):
            errors.append(f'{name}: nonempty unique IDs required')
        return value
    if type(catalog.get('schema_version')) is not int or catalog['schema_version'] != 1:
        errors.append('schema_version must be 1')
    if catalog.get('kind') != 'model_service_registry_design' or catalog.get('phase') != 'design_documentation_only':
        errors.append('Catalog must remain model_service_registry_design / design_documentation_only')
    if not text(catalog.get('revision')):
        errors.append('Catalog revision is required')
    protocols, presets, acceptance = rows('protocols'), rows('presets'), rows('acceptance')
    authority = catalog.get('authority')
    if not isinstance(authority, dict):
        authority = {}
        errors.append('authority object required')
    counts = catalog.get('counts')
    expected = {'protocols': 13, 'presets': 21, 'excluded_protocols': 1}
    if not isinstance(counts, dict) or any(type(counts.get(k)) is not int or counts[k] != value for k, value in expected.items()):
        errors.append('counts must preserve the authorized 13 protocols /21 presets /1 excluded protocol')
    for name, value, count in [('protocol_ids', protocols, 13), ('preset_ids', presets, 21)]:
        authorized = authority.get(name)
        actual = [row.get('id') for row in value]
        if not strings(authorized) or len(authorized) != count or len(actual) != count or actual != authorized:
            errors.append(f'{name}: exact authoritative IDs/order and count are required')
    if authority.get('excluded_protocol_ids') != ['tencent_tmt_legacy'] or authority.get('excluded_direction_ids') != ['tencent-legacy']:
        errors.append('TMT exclusion must explicitly retain tencent_tmt_legacy /tencent-legacy')
    mapping = {row.get('id'): row for row in protocols if text(row.get('id'))}
    for row in protocols:
        identity = str(row.get('id'))
        for field in ['id', 'label', 'family', 'auth_kind', 'auth_description', 'model_identity', 'wire_output', 'source_state', 'official_source']:
            if not text(row.get(field)):
                errors.append(f'protocol {identity}: {field} is required')
        if not positive(row.get('contract_revision')):
            errors.append(f'protocol {identity}: positive contract_revision required')
        if row.get('family') not in ('llm', 'translation'):
            errors.append(f'protocol {identity}: unknown family')
        purposes = row.get('allowed_purposes')
        if not strings(purposes) or not set(purposes).issubset(PURPOSES):
            errors.append(f'protocol {identity}: invalid allowed_purposes')
        elif row.get('family') == 'translation' and purposes != ['translation']:
            errors.append(f'protocol {identity}: translation protocol cannot gain learning purposes')
        if row.get('adapter_acceptance') != 'not_run' or row.get('live_acceptance') != 'not_run':
            errors.append(f'protocol {identity}: new adapter/live acceptance must remain not_run')
        state = 'existing_source_requires_registry_migration' if identity in EXISTING_PROTOCOLS else 'not_implemented'
        if row.get('source_state') != state:
            errors.append(f'protocol {identity}: source_state must preserve current-source versus planned distinction')
        try:
            url = urlsplit(str(row.get('official_source', '')))
            valid_url = url.scheme == 'https' and bool(url.netloc)
        except ValueError:
            valid_url = False
        if not valid_url:
            errors.append(f'protocol {identity}: official_source must be a public HTTPS reference')
        if re.search(r'(?:^|[^a-z])tmt(?:$|[^a-z])|tc3', identity + ' ' + str(row.get('auth_kind', '')), re.I):
            errors.append(f'protocol {identity}: excluded TMT/TC3 protocol returned')
    directions = [row.get('direction_id') for row in presets]
    if not all(text(item) for item in directions) or len(set(str(item) for item in directions)) != len(directions):
        errors.append('presets: unique direction_id required')
    for row in presets:
        identity = str(row.get('id'))
        for field in ['id', 'label', 'direction_id', 'provider_id', 'protocol_id', 'compatibility_profile_id']:
            if not text(row.get(field)):
                errors.append(f'preset {identity}: {field} is required')
        for field in ['adapter_contract_revision', 'profile_revision', 'preset_revision']:
            if not positive(row.get(field)):
                errors.append(f'preset {identity}: positive {field} required')
        protocol = mapping.get(row['protocol_id']) if text(row.get('protocol_id')) else None
        if protocol is None:
            errors.append(f'preset {identity}: dangling protocol_id')
        else:
            if row.get('adapter_contract_revision') != protocol.get('contract_revision'):
                errors.append(f'preset {identity}: adapter contract revision differs from protocol')
            purposes = row.get('allowed_purposes')
            protocol_purposes = protocol.get('allowed_purposes')
            if not strings(purposes) or not strings(protocol_purposes) or not set(purposes).issubset(set(protocol_purposes)):
                errors.append(f'preset {identity}: allowed_purposes must be a protocol subset')
        if row.get('implementation_status') != 'planned' or row.get('acceptance_status') != 'not_run':
            errors.append(f'preset {identity}: implementation must stay planned and acceptance not_run')
        if row.get('direction_id') == 'tencent-legacy' or row.get('protocol_id') == 'tencent_tmt_legacy' or re.search(r'(?:^|[^a-z])tmt(?:$|[^a-z])|tc3', identity, re.I):
            errors.append(f'preset {identity}: excluded TMT route returned')
    tokenhub = {row.get('direction_id'): row for row in presets if str(row.get('direction_id', '')).startswith('tencent-tokenhub-')}
    if set(tokenhub) != {'tencent-tokenhub-chat', 'tencent-tokenhub-translation'}:
        errors.append('TokenHub Chat and dedicated translation must both remain separate routes')
    else:
        for direction, protocol in [('tencent-tokenhub-chat', 'openai_compatible'), ('tencent-tokenhub-translation', 'tencent_tokenhub_translation')]:
            row = tokenhub[direction]
            if row.get('protocol_id') != protocol or row.get('provider_id') != 'builtin.tencent-tokenhub' or row.get('allowed_purposes') != ['translation']:
                errors.append(f'TokenHub {direction}: protocol/provider and translation-only purpose must remain exact')
    for row in acceptance:
        if row.get('status') != 'not_run':
            errors.append(f"acceptance {row.get('id')}: new product acceptance must remain not_run")
        if not all(text(row.get(field)) for field in ['label', 'assertion', 'maintenance']) or not strings(row.get('source_requirement_ids')):
            errors.append(f"acceptance {row.get('id')}: traceable assertion/requirements/maintenance required")
    state = catalog.get('implementation_state', {})
    if not isinstance(state, dict) or not strings(state.get('production_protocol_source')) or set(state['production_protocol_source']) != EXISTING_PROTOCOLS or any(state.get(key) != 'not_implemented' for key in ['registry', 'new_protocols', 'typed_credential_migration']) or state.get('product_acceptance') != 'not_run':
        errors.append('implementation_state must preserve three existing source protocols; new implementation/acceptance is not completed')
    if catalog.get('source_documents') != [path for _, _, path in DOCS]:
        errors.append('source_documents must match the exact six current Markdown sources')
    for _, _, relative in DOCS:
        try:
            _safe(Path(root).resolve(), relative)
        except ValueError as error:
            errors.append(str(error))
    return errors


def _check_snapshot(root: Path, catalog: dict, body: str) -> list[str]:
    errors = validate_catalog(root, catalog)
    if errors:
        return errors
    for name, renderer in [('protocols', render_protocol_table), ('presets', render_preset_table)]:
        start, end = MARKERS[name]
        if body.count(start) != 1 or body.count(end) != 1 or body.index(start) >= body.index(end):
            errors.append(f'{name}: exactly one ordered generated marker pair required')
            continue
        actual = body.split(start, 1)[1].split(end, 1)[0]
        expected = '\n' + renderer(catalog) + '\n'
        if actual != expected:
            errors.append(f'{name}: generated Markdown table drift; regenerate only the controlled marker content after review')
    return errors


def check(root: Path = ROOT) -> list[str]:
    root = Path(root).resolve()
    try:
        return _check_snapshot(root, load_catalog(root), _safe(root, PRIMARY).read_text(encoding='utf-8'))
    except (OSError, ValueError, KeyError, TypeError, UnicodeError) as error:
        return [f'Model-service design documents cannot be checked: {error}']


def _output_path(root: Path, output: Path) -> Path:
    lexical_root = root.absolute()
    root = lexical_root.resolve()
    output = output if output.is_absolute() else lexical_root / output
    # Accept the caller's root spelling (macOS /var can alias /private/var),
    # then inspect every component *inside* the real root without following it.
    try:
        relative = output.relative_to(lexical_root)
    except ValueError:
        try:
            relative = output.relative_to(root)
        except ValueError as error:
            raise ValueError('Report output must remain inside this root reports/ or .build/') from error
    output = root / relative
    for parent in (output, *output.parents):
        if parent == root:
            break
        if parent.is_symlink():
            raise ValueError('Report output symlinks are forbidden')
    output = output.resolve()
    if not any(output.is_relative_to(root / name) and output != root / name for name in ('reports', '.build')):
        raise ValueError('Report output must be a fresh child of this root reports/ or .build/')
    if output.exists():
        raise ValueError('Refusing to overwrite an existing report/evidence directory')
    return output


def _markdown(root: Path, report_tools: Path | None):
    candidates = [Path(report_tools)] if report_tools is not None else [root / '.build/report-tools', *[parent / '.build/report-tools' for parent in ROOT.parents]]
    dependency = next((p.resolve() for p in candidates if (p / 'markdown/__init__.py').is_file()), None)
    if dependency is None:
        raise ValueError('Existing .build/report-tools markdown dependency is required; no installation is performed')
    sys.path.insert(0, str(dependency))
    return importlib.import_module('markdown')


class _SourceLinks(HTMLParser):
    def __init__(self, root: Path, source: Path):
        super().__init__(convert_charrefs=False)
        self.root, self.source, self.parts = root, source, []
    def handle_starttag(self, tag, attrs):
        rewritten = []
        for key, value in attrs:
            if key in ('href', 'src') and value is not None:
                parsed = urlsplit(value)
                if parsed.scheme or parsed.netloc:
                    if parsed.scheme not in ('http', 'https', 'mailto'):
                        key, value = 'data-source-reference', value
                else:
                    target = (self.source.parent / unquote(parsed.path)).resolve() if parsed.path else self.source
                    if target.is_relative_to(self.root) and target.exists():
                        value = target.as_uri() + ('#' + parsed.fragment if parsed.fragment else '')
                    else:
                        key, value = 'data-source-reference', value
            rewritten.append(key + ('' if value is None else '="' + html.escape(value, quote=True) + '"'))
        self.parts.append('<' + tag + (' ' + ' '.join(rewritten) if rewritten else '') + '>')
    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        self.parts[-1] = self.parts[-1][:-1] + ' />'
    def handle_endtag(self, tag): self.parts.append('</' + tag + '>')
    def handle_data(self, data): self.parts.append(data)
    def handle_entityref(self, name): self.parts.append('&' + name + ';')
    def handle_charref(self, name): self.parts.append('&#' + name + ';')
    def handle_comment(self, data): self.parts.append('<!--' + data + '-->')


def _architecture() -> str:
    boxes = [(20, 25, '产品 UI', '草稿 / 测试 / 应用 / 启用'), (330, 25, '窄 Tauri 适配', '命令 / DTO / 权限'),
             (640, 25, 'Rust 业务权威', '对象身份 / 取消 / 缓存'), (20, 185, '版本化注册设计', 'Protocol / Profile / Preset'),
             (330, 185, '协议适配设计', '13 协议 /21 路由'), (640, 185, '凭据与网络端口', '系统 Vault /受控 HTTP')]
    content = '<svg viewBox="0 0 950 305" role="img" aria-labelledby="architecture-title"><title id="architecture-title">模型服务注册目标架构，尚未实现</title><g fill="none" stroke="#7b8ba4" stroke-width="2"><path d="M290 65H330M600 65H640M775 105V155H155V185M155 155H465V185M775 155V185"/></g>'
    for x, y, title, subtitle in boxes:
        content += f'<rect x="{x}" y="{y}" width="270" height="80" rx="12" fill="#eef2f7" stroke="#a5b2c4"/><text x="{x+135}" y="{y+32}" text-anchor="middle">{title}</text><text x="{x+135}" y="{y+57}" text-anchor="middle" font-size="12">{subtitle}</text>'
    return content + '</svg>'


def build_report(root: Path, output: Path, report_tools: Path | None = None) -> dict:
    output = _output_path(Path(root), Path(output))
    root = Path(root).resolve()
    snapshot = {relative: _safe(root, relative).read_bytes() for relative in [CATALOG, *[path for _, _, path in DOCS]]}
    catalog = _parse_catalog(snapshot[CATALOG])
    errors = _check_snapshot(root, catalog, snapshot[PRIMARY].decode('utf-8'))
    if errors:
        raise ValueError('Document checks failed: ' + '; '.join(errors))
    def unchanged():
        if any(_safe(root, relative).read_bytes() != raw for relative, raw in snapshot.items()):
            raise ValueError('Source documents changed during export; refusing a passed report manifest')
    markdown = _markdown(root, report_tools)  # Only export loads this existing dependency.
    sources = []
    chapters = []
    for anchor, title, relative in DOCS:
        source = _safe(root, relative)
        raw = snapshot[relative]
        sources.append({'path': relative, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()})
        rendered = markdown.markdown(raw.decode(), extensions=['tables', 'fenced_code', 'toc'])
        links = _SourceLinks(root, source)
        links.feed(rendered)
        chapters.append(f'<section id="{anchor}"><h2>{html.escape(title)}</h2><p><a href="{source.as_uri()}">仓库源文档 · {html.escape(relative)}</a></p>' + ''.join(links.parts) + '</section>')
    raw = snapshot[CATALOG]
    sources.append({'path': CATALOG, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()})
    navigation = ''.join(f'<a href="#{anchor}">{html.escape(title)}</a>' for anchor, title, _ in DOCS)
    tree = '\n'.join(['docs/', '├── architecture/', '│   ├── japanese-study-and-llm.md  ← 主技术设计与受控矩阵', '│   ├── model-service-registry-design.json  ← 13/21规划目录', '│   ├── first-product-engineering-baseline.md', '│   └── database-design.md', '├── product/', '│   ├── model-configuration-prd.md', '│   └── README.md', '└── development/engineering-gates.md', '', 'crates/app-core/                     ← 保持Rust业务权威边界', 'crates/app-infra/src/providers/      ← 后续协议扩展归属，尚未实施', 'scripts/report/build_model_service_design_report.py'])
    document = '<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>模型服务注册设计 · 13协议／21路由</title><style>body{font:15px/1.8 -apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#243046;background:#f4f6fa;margin:0}main{max-width:1160px;margin:auto;padding:36px 24px}h1{font-size:30px}h2{scroll-margin-top:20px}nav{display:flex;flex-wrap:wrap;gap:8px 20px}a{color:#285c99;overflow-wrap:anywhere}section{background:#fff;margin:28px 0;padding:26px;border:1px solid #dce2eb;border-radius:14px;overflow:auto}table{border-collapse:collapse;width:100%;font-size:12px}td,th{border:1px solid #dce2eb;padding:8px;min-width:82px;overflow-wrap:anywhere}pre{padding:18px;background:#edf1f7;overflow:auto}code{overflow-wrap:anywhere}svg{width:100%;height:auto;min-width:630px}.diagram{overflow:auto}.notice{padding:16px;border-left:4px solid #8b9bb4;background:#e8edf5}img{max-width:100%}</style><main><header><h1>模型服务注册设计 · 13协议／21路由</h1><p class="notice">本轮仅同步设计文档。TMT明确排除，TokenHub Chat与专用翻译分别保留；新注册实现、适配与真实服务验收均未完成。文档检查通过不等于产品通过。</p><nav>' + navigation + '</nav></header><section id="target-architecture"><h2>目标架构与边界</h2><div class="diagram">' + _architecture() + '</div><p>图为目标职责，13/21扩展尚未实施；当前产品身份与历史结果保持原记录。</p><h3>来源与目录</h3><pre>' + html.escape(tree) + '</pre></section>' + ''.join(chapters) + '<footer>只导出本轮六份Markdown及目录摘要；未复制游戏证据、资源、旧包或批准截图。来源hash见 source-manifest.json。</footer></main></html>'
    unchanged()
    output.mkdir(parents=True, exist_ok=False)
    (output / 'index.html').write_text(document, encoding='utf-8')
    unchanged()
    manifest = {'schema_version': 1, 'kind': 'model_service_design_document_export', 'at': datetime.now(timezone.utc).isoformat(),
                'catalog_revision': catalog['revision'], 'scope': 'documentation_only', 'source_root': str(root), 'sources': sources,
                'counts': catalog['counts'], 'documentation_checks': 'passed', 'product_tests_executed': False,
                'product_acceptance': 'not_run', 'protocol_implementation_promoted': False,
                'files': [{'path': 'index.html', 'bytes': len(document.encode()), 'sha256': hashlib.sha256(document.encode()).hexdigest()}]}
    (output / 'source-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    try:
        unchanged()
    except (OSError, ValueError):
        manifest['documentation_checks'] = 'failed_source_changed'
        (output / 'source-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        raise
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check', action='store_true')
    mode.add_argument('--output', type=Path)
    parser.add_argument('--report-tools', type=Path)
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args()
    try:
        if args.check:
            errors = check(args.root)
            result = {'status': 'failed' if errors else 'passed_documentation_only', 'errors': errors, 'scope': 'design-document consistency only; no product acceptance', 'product_acceptance': 'not_run'}
        else:
            manifest = build_report(args.root, args.output, args.report_tools)
            errors = []
            result = {'status': 'exported_documentation_only', 'output': str(args.output), 'source_count': len(manifest['sources']), 'product_acceptance': 'not_run'}
    except (OSError, ValueError, KeyError, TypeError, ImportError) as error:
        errors = [str(error)]
        result = {'status': 'failed', 'errors': errors, 'product_acceptance': 'not_run'}
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return int(bool(errors))


if __name__ == '__main__':
    raise SystemExit(main())
