#!/usr/bin/env python3
"""Render the engineering decision documents; never builds or runs the product."""
from pathlib import Path
import csv
import hashlib
import html
import json
import re
import shutil
import sys
from urllib.parse import quote, unquote, urlsplit
import zipfile
from engineering_directory_map import directory_map_svg

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "reports/first-product-engineering-20261018"
sys.path.insert(0, str(ROOT / ".build/report-tools"))
import markdown

DOCS = [
    ("baseline", "功能与研发架构", "docs/architecture/first-product-engineering-baseline.md"),
    ("storage-acceptance", "存储模块验收规范与执行器", "docs/development/storage-acceptance.md"),
    ("database", "数据库字段、事务与生命周期设计", "docs/architecture/database-design.md"),
    ("test-operations", "测试供给与长期维护", "docs/development/first-product-test-operations.md"),
    ("layout", "目录与文件组织", "docs/architecture/engineering-layout.md"),
    ("decision", "决策与取舍", "docs/architecture/ADR-004-first-product-engineering.md"),
    ("governance", "后续迭代如何保持一致", "docs/development/iteration-governance.md"),
    ("gates", "OpenSpec、代码与编译的可执行门禁", "docs/development/engineering-gates.md"),
    ("test-plan", "首版逐层测试与上线准入", "docs/development/first-product-test-plan.md"),
    ("iteration-template", "每次开工与交付的迭代卡", "docs/development/iteration-template.md"),
]
TABLES = [
    ("database-tables", "数据库表目录与职责", "docs/architecture/database/table-catalog.csv"),
    ("database-fields", "数据库逐字段字典", "docs/architecture/database/field-dictionary.csv"),
    ("modules", "16 个模块与职责", "docs/architecture/engineering-modules.csv"),
    ("components", "34 个组件族与复用", "docs/architecture/engineering-components.csv"),
    ("test-matrix", "128 个测试定义与执行变体", "docs/development/first-product-test-matrix.csv"),
]

STYLE = """
:root{color-scheme:dark;font-family:-apple-system,BlinkMacSystemFont,'PingFang SC','Hiragino Sans',sans-serif;background:#141720;color:#e9e9ef;font-size:15px;line-height:1.8}*{box-sizing:border-box}body{margin:0;background:radial-gradient(ellipse at 85% 0,#29394a60,transparent 40%),#141720}a{color:#c4c5cf;text-underline-offset:4px}button,input{font:inherit;color:inherit}main,header,footer{width:min(1320px,100%);margin:auto;padding:30px 34px}.eyebrow{font-size:12px;letter-spacing:1.4px;color:#b0b2c0}h1{font-size:40px;line-height:1.3;font-weight:600;letter-spacing:-1px;margin:14px 0 16px}h2{font-size:25px;line-height:1.5;margin:30px 0 14px;scroll-margin-top:24px}h3{font-size:19px;margin:26px 0 12px}p,li{overflow-wrap:anywhere}p{color:#b0b2c0}strong{color:#e9e9ef}header{padding-top:52px}.lead{max-width:900px;font-size:17px}.status{display:inline-block;border:1px solid #a0a0ad24;border-radius:7px;padding:3px 11px;background:#9999a910;font-size:12px;color:#b0b2c0}.metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin:26px 0}.metrics>div{background:#3a3a442d;border:1px solid #a0a0ad24;border-radius:13px;padding:16px 20px}.metrics b{display:block;font-size:26px;font-weight:550}.metrics span{font-size:12px;color:#b0b2c0}.summary-grid{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin:22px 0}.summary-grid section{margin:0}.summary-grid h2{font-size:18px;margin:0 0 9px}.chapter{margin:0 0 24px;padding:24px 28px;background:rgba(35,38,49,.82);border:1px solid #070911e0;border-radius:16px;box-shadow:inset 0 1px 0 #ffffff10,0 9px 32px #0003;min-width:0;overflow:hidden}.chapter>h2:first-child{margin-top:0}.chapter p:last-child{margin-bottom:0}nav{display:flex;flex-wrap:wrap;gap:9px 20px;margin:26px 0 0}nav a{font-size:13px}.table-wrap{overflow:auto;max-width:100%;border-radius:7px;margin:14px 0}table{border-collapse:collapse;width:100%;font-size:12px;line-height:1.7}th,td{vertical-align:top;text-align:left;padding:10px 13px;border:1px solid #a0a0ad24;min-width:100px;max-width:390px;overflow-wrap:anywhere}th{background:#9999a912;color:#e9e9ef;white-space:nowrap}td{color:#b0b2c0}tr[hidden]{display:none}pre{padding:19px;border:1px solid #a0a0ad24;border-radius:10px;background:#14172270;overflow:auto;font-size:12px;line-height:1.75}code{font-family:'SFMono-Regular',Consolas,monospace;font-size:.86em;padding:2px 4px;border-radius:3px;background:#9999a90f}pre code{font-size:inherit;background:none;padding:0}.toolbar{display:flex;align-items:center;gap:16px;flex-wrap:wrap;margin:18px 0}.toolbar label{display:flex;gap:10px;align-items:center;color:#b0b2c0;font-size:13px}.toolbar input{width:320px;max-width:100%;background:#14172299;border:1px solid #a0a0ad3c;border-radius:7px;padding:7px 10px;min-width:0}.row-count{color:#9495a5;font-size:12px}.diagram{display:block;width:100%;height:auto;margin:22px 0;border:1px solid #a0a0ad24;border-radius:13px;background:#1d202c}.diagram text{font-family:-apple-system,BlinkMacSystemFont,'PingFang SC',sans-serif;fill:#e9e9ef}.diagram .detail{fill:#b0b2c0;font-size:12px}.diagram .title{font-size:16px;font-weight:550}.reference-grid{display:grid;grid-template-columns:1fr 1fr;gap:16px}.reference-grid figure{margin:0;min-width:0}.reference-grid img{width:100%;height:auto;display:block;border-radius:10px;border:1px solid #a0a0ad24}.reference-grid figcaption{font-size:12px;color:#b0b2c0;margin:9px 0 18px}.note{border-left:2px solid #9495a5;padding:7px 0 7px 16px;color:#b0b2c0}footer{color:#9495a5;font-size:12px;padding-bottom:64px}.section-title{display:flex;align-items:baseline;gap:14px;flex-wrap:wrap}.section-title h2{margin:0}.chapter a{overflow-wrap:anywhere}details{border-top:1px solid #a0a0ad24;padding-top:12px;margin-top:12px}summary{cursor:pointer;color:#c4c5cf}a:focus-visible,input:focus-visible,summary:focus-visible{outline:2px solid #c4c5cf;outline-offset:4px}@media(max-width:800px){main,header,footer{padding:24px 18px}header{padding-top:34px}h1{font-size:30px}.chapter{padding:20px 16px}.summary-grid,.reference-grid{grid-template-columns:1fr}.metrics{grid-template-columns:1fr 1fr;gap:9px}.metrics>div{padding:12px 15px}h2{font-size:22px}.toolbar label{width:100%}.toolbar input{width:100%}table{font-size:12px}.diagram .title{font-size:17px}.diagram .detail{font-size:12px}}@media print{body{background:white;color:#111}p,td,th,strong{color:#111}.chapter{box-shadow:none;border:0;background:none;padding:0}.toolbar,nav{display:none}header,main{width:100%;padding:0}pre{white-space:pre-wrap;overflow:visible}.table-wrap{overflow:visible}table{font-size:9px}td{min-width:0}a{color:inherit}}
"""

STYLE += """
.directory-viewport{overflow-x:auto;border:1px solid #a0a0ad24;border-radius:14px;margin:20px 0 12px;background:#1a1e29}.directory-map{display:block;width:100%;min-width:1000px;height:auto}.map-links{display:flex;gap:20px;flex-wrap:wrap;font-size:13px}.directory-viewport:focus-visible{outline:2px solid #c4c5cf;outline-offset:4px}@media print{.directory-map{min-width:0}.directory-viewport{overflow:visible;break-inside:avoid}}
"""


def diagram():
    boxes = [
        (20, 20, "React · 四个窗口", "主窗 / HUD / 设置 / 学习"),
        (335, 20, "Tauri · 窄适配", "命令 / 订阅 / 系统窗口"),
        (650, 20, "Rust app-core", "业务用例与唯一权威状态"),
        (20, 170, "HookBackend", "hook-client / 身份 / 进程监督"),
        (335, 170, "LanguagePort", "本机独立语言 worker"),
        (650, 170, "其他 I/O 端口", "SQLite / HTTP / CredentialVault"),
        (20, 310, "Windows broker + Core", "同 Bottle · 同位数 · ABI 1"),
        (335, 310, "MeCab + UniDic + 词典", "离线随包 · 不进入游戏线程"),
        (650, 310, "用户数据与配置服务", "版本化 / 有界任务 / 系统凭据"),
    ]
    parts = ['<svg class="diagram" role="img" aria-labelledby="arch-title arch-description" viewBox="0 0 950 445" xmlns="http://www.w3.org/2000/svg"><title id="arch-title">首版进程与模块调用关系</title><desc id="arch-description">React窗口通过Tauri调用Rust应用核心。基础设施适配器连接Hook、独立语言worker、数据库和模型服务。图中为整体职责边界，当前实现与残余差距见实现状态章节，不能由此图推定全部完成。</desc><defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M0 0L10 5L0 10z" fill="#9495a5"/></marker></defs><g fill="none" stroke="#9495a5" stroke-width="1.5" marker-end="url(#arrow)"><path d="M300 58H330"/><path d="M615 58H645"/><path d="M790 96V135H160V164"/><path d="M790 135H475V164"/><path d="M790 135V164"/><path d="M160 246V304"/><path d="M475 246V304"/><path d="M790 246V304"/></g>']
    for x, y, title, detail in boxes:
        parts.append(f'<rect x="{x}" y="{y}" width="280" height="76" rx="10" fill="#3a3a4478" stroke="#a0a0ad30"/><text x="{x+140}" y="{y+31}" text-anchor="middle" class="title">{html.escape(title)}</text><text x="{x+140}" y="{y+55}" text-anchor="middle" class="detail">{html.escape(detail)}</text>')
    parts.append('<text x="475" y="421" text-anchor="middle" class="detail">模型、词典和数据库不进入 Core；目标进程与会话身份分别建模。</text></svg>')
    return ''.join(parts)


def rewrite_links(body, source):
    def replace(match):
        target = html.unescape(match.group(1))
        parts = urlsplit(target)
        if parts.scheme:
            return match.group(0)
        if target.startswith('#'):
            return 'href="references/' + quote(source, safe='/') + html.escape(target, quote=True) + '"'
        absolute = (ROOT / Path(source).parent / unquote(parts.path)).resolve()
        if not absolute.is_relative_to(ROOT):
            raise ValueError(f"Unexpected external local link: {source} {target}")
        dest = "references/" + quote(str(absolute.relative_to(ROOT)), safe='/')
        if parts.query:
            dest += '?' + parts.query
        if parts.fragment:
            dest += '#' + parts.fragment
        return f'href="{html.escape(dest, quote=True)}"'
    return re.sub(r'href="([^"]+)"', replace, body)


def render_document(key, title, source, replace_mermaid=True):
    text = (ROOT / source).read_text()
    text = re.sub(r'^# .+\n', '', text, count=1)
    if replace_mermaid:
        text = re.sub(r'```mermaid\n.*?\n```', diagram(), text, flags=re.S)
    body = markdown.markdown(text, extensions=['tables', 'fenced_code', 'sane_lists'])
    body = rewrite_links(body, source)
    body = re.sub(r'<table>(.*?)</table>', r'<div class="table-wrap"><table>\1</table></div>', body, flags=re.S)
    return f'<article class="chapter" id="{key}"><h2>{html.escape(title)}</h2>{body}</article>'


def render_table(key, title, source):
    if key == 'database-fields':
        return render_database_fields(key, title, source)
    with (ROOT / source).open(newline='') as file:
        reader = csv.DictReader(file)
        rows, fields = list(reader), reader.fieldnames
    head = ''.join(f'<th>{html.escape(name)}</th>' for name in fields)
    body = ''.join('<tr>' + ''.join(f'<td>{html.escape(row[name])}</td>' for name in fields) + '</tr>' for row in rows)
    return f'<section class="chapter" id="{key}"><div class="section-title"><h2>{title}</h2><a href="references/{source}" download>下载 CSV</a></div><p>主责、契约、复用和状态均可检索；模块/组件状态来自当前源码核对，测试矩阵仍保留计划身份。</p><div class="toolbar"><label>筛选 <input type="search" aria-label="筛选{title}" data-filter="{key}-table" placeholder="输入模块、Owner、FP 或关键词"></label><span class="row-count" id="{key}-count">{len(rows)} / {len(rows)} 项</span></div><div class="table-wrap"><table id="{key}-table"><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div></section>'


def render_database_fields(key, title, source):
    with (ROOT / source).open(newline='') as stream:
        rows = list(csv.DictReader(stream))
    boundary_fields = {field['id']: field for field in json.loads((ROOT / 'docs/architecture/database/field-boundaries.json').read_text())['fields']}
    parts = []
    for row in rows:
        identity = html.escape(row['database'] + '.' + row['table'] + '.' + row['field'])
        fks = json.loads(row['foreign_keys'])
        references = '; '.join('(' + ', '.join(fk['columns']) + ') → ' + fk['references_table'] + '(' + ', '.join(str(value) for value in fk['references_columns']) + '); DELETE ' + fk['on_delete'] for fk in fks)
        detail = '<details><summary>声明与约束</summary><code>' + html.escape(row['check_or_declaration']) + '</code><p>' + html.escape(references or '无外键') + '</p><p>' + html.escape(row['owner'] + ' · ' + row['requirements'] + ' · ' + row['source'] + ':' + row['source_line']) + '</p></details>'
        boundary = boundary_fields[row['database'] + '.' + row['table'] + '.' + row['field']]
        detail += f'<p>字段边界：{len(boundary["probes"])} 个实际设计探针；生产Rust映射须绑定当次源码及正式包验收。</p>'
        cells = [f'<code>{identity}</code>', html.escape(row['type']), '可空' if row['nullable'] == 'true' else '必填', html.escape(row['primary_key_position']) if row['primary_key_position'] != '0' else '—', html.escape(row['default']) or '—', html.escape(row['description']), detail]
        parts.append('<tr>' + ''.join('<td>' + value + '</td>' for value in cells) + '</tr>')
    headers = ['库 · 表 · 字段', '类型', '空值', '主键序位', '默认值', '字段含义', '外键与约束']
    heading = ''.join('<th>' + name + '</th>' for name in headers)
    return f'<section class="chapter" id="{key}"><div class="section-title"><h2>{title}</h2><a href="references/{source}" download>下载完整字段 CSV</a></div><p>逐字段检索；展开可看完整声明、CHECK、复合外键、Owner和源行号。主键/默认值“—”表示未声明，并不表示字段允许NULL。窄屏表格可横向滚动。</p><div class="toolbar"><label>筛选 <input type="search" aria-label="筛选{title}" data-filter="{key}-table" placeholder="输入表名、字段、用途或说明"></label><span class="row-count" id="{key}-count">{len(rows)} / {len(rows)} 项</span></div><div class="table-wrap"><table id="{key}-table"><thead><tr>{heading}</tr></thead><tbody>{"".join(parts)}</tbody></table></div></section>'


def storage_enforcement_section(root):
    """Render verified tooling coverage separately from blocked product acceptance."""
    root = Path(root)
    proof = validate_current_database_proof(root, root / '.build/quality/database-design-verification.json')
    boundary = json.loads((root / 'docs/architecture/database/field-boundaries.json').read_text())
    storage = json.loads((root / 'docs/development/storage-acceptance.json').read_text())
    counts = boundary['counts']
    rows = ''.join('<tr><td><code>' + html.escape(item['id']) + '</code></td><td>' + html.escape(item['title']) + '</td><td>' + html.escape(item['owner']) + '</td><td>' + html.escape(', '.join(item['execution_ids'])) + '</td><td>not_run · 缺正式adapter</td></tr>' for item in storage['checks'])
    return f'''<section class="chapter" id="storage-enforcement"><h2>存储验收与字段边界：由程序阻断差异</h2><p><strong>设计层已实跑：{counts["fields"]} 个字段、{counts["probes"]} 个边界探针、{proof["storage_boundary_assertions"]} 个断言。</strong> {counts["check_constraints"]} 个CHECK中，{counts["isolated_checks"]} 个有独立反例，{counts["logically_redundant_checks"]} 个有明确逻辑冗余证书与联合反例。字段投影保留原始列/CHECK/唯一性，有意隔离FK与业务trigger；完整业务与真实迁移另行验收。</p><p>实际数据库以只读程序对照独立冻结schema：字段类型/空值/default/PK、复合外键、删除动作、索引、CHECK和触发器均纳入，漏字段或约束漂移非零退出。字段边界覆盖合法/非法、NULL、无损转换、JSON、u64、NUL及已声明界限；未冻结容量政策保持待定。</p><p class="note"><strong>产品存储验收：blocked。</strong> 当前无正式生产adapter，PRE-14支持升级/拒绝/恢复版本矩阵仍待冻结。未来必须实际编译并运行Rust存储测试目标、逐条运行迁移路线及所有字段映射/边界；仅填测试名、旧回执、skip或设计DDL克隆均拒绝。下面是既有产品组合的{len(storage["checks"])}项子验收，全部保持未执行。</p><pre><code>npm run verify:storage-contract  # 设计/字段/验收登记，可执行
npm run verify:storage           # 产品存储验收，当前应blocked</code></pre><div class="table-wrap"><table id="storage-acceptance-table"><thead><tr><th>子检查</th><th>验收内容</th><th>Owner</th><th>既有组合</th><th>产品状态</th></tr></thead><tbody>{rows}</tbody></table></div></section>'''


def current_source_tree_hash(root):
    sys.path.insert(0, str(root / 'scripts/quality'))
    from gate_core import source_files
    return hashlib.sha256(json.dumps(source_files(root), sort_keys=True).encode()).hexdigest()


def validate_current_database_proof(root, summary_path):
    """Reject stale/failed/unbound evidence before rendering any passed claim."""
    root = Path(root).resolve()
    summary = json.loads(Path(summary_path).read_text())

    def require(condition, message):
        if not condition:
            raise ValueError('Database report proof: ' + message)

    def local_file(relative):
        require(isinstance(relative, str) and relative and not Path(relative).is_absolute(), 'relative evidence path required')
        path = root / relative
        require('..' not in Path(relative).parts and path.resolve().is_relative_to(root), 'evidence path escapes workspace')
        require(path.is_file() and not any(p.is_symlink() for p in [path, *path.parents] if p != root.parent), 'missing or symlink evidence file')
        return path

    require(summary.get('status') == 'passed' and summary.get('product_tests_executed') is False, 'summary is failed or claims product execution')
    receipt_path = local_file(summary.get('receipt_path'))
    require(hashlib.sha256(receipt_path.read_bytes()).hexdigest() == summary.get('receipt_sha256'), 'receipt hash mismatch')
    receipt = json.loads(receipt_path.read_text())
    require(receipt.get('status') == 'passed' and receipt.get('profile') == 'development' and receipt.get('product_runtime_tests_executed') is False, 'receipt is not a passed engineering development run')
    actual_tree = current_source_tree_hash(root)
    require(receipt.get('source_tree_sha256') == actual_tree == summary.get('source_tree_sha256'), 'stale source tree')
    baseline_path = root / 'docs/development/iteration-baseline.json'
    baseline = json.loads(baseline_path.read_text())
    require(summary.get('baseline_id') == baseline['baseline_id'] and summary.get('baseline_sha256') == hashlib.sha256(baseline_path.read_bytes()).hexdigest(), 'baseline identity mismatch')
    steps = receipt.get('steps', [])
    require(isinstance(steps, list) and all(isinstance(row, dict) for row in steps), 'invalid steps')
    by_id = {row.get('id'): row for row in steps}
    require(len(by_id) == len(steps), 'duplicate check IDs')
    required = {'change-contract','baseline-integrity','source-scope','database-design-contract','database-design-export-identity','database-design-tests','typescript-compile','rust-compile','scenario-check-execution','storage-schema-contract','storage-boundaries','storage-boundary-execution','storage-acceptance-contract','storage-schema-tests','storage-boundary-tests','storage-acceptance-tests'}
    require(required <= by_id.keys(), 'required database/compilation checks did not execute')
    require(all(row.get('status') == 'passed' and row.get('errors') == [] for row in steps), 'one or more checks failed')
    for check_id in ['database-design-export-identity','database-design-tests','typescript-compile','rust-compile','storage-boundaries','storage-schema-tests','storage-boundary-tests','storage-acceptance-tests']:
        row = by_id[check_id]
        require(row.get('exit_code') == 0 and isinstance(row.get('command'), list) and row['command'], 'check exit/command invalid: ' + check_id)
    db = by_id['database-design-tests']
    require(db['command'][-1] == 'test_database_design.DatabaseDesignTests', 'database test selector mismatch')
    sys.path.insert(0, str(root / 'scripts/quality'))
    from verify import validate_test_output
    for row in steps:
        if 'log' in row:
            local_file(row['log'])
        count = row.get('test_count')
        if count is not None:
            require(type(count) is int and count > 0, 'empty/invalid test count')
            log = local_file(row.get('log')).read_text()
            framework = 'python' if '-m' in row.get('command', []) and 'unittest' in row.get('command', []) else 'node'
            actual_count, errors = validate_test_output(framework, log)
            passed_end = bool(re.search(r'^OK\s*$', log, re.M)) if framework == 'python' else not errors
            require(not errors and actual_count == count and passed_end, 'actual test log does not match passed count: ' + row['id'])
    require(summary.get('database_design_tests') == db.get('test_count') and type(db.get('test_count')) is int and db['test_count'] > 0, 'database count mismatch')
    for check_id, suite in [('storage-schema-tests','test_storage_schema.StorageSchemaTests'), ('storage-boundary-tests','test_storage_boundaries.StorageBoundaryTests'), ('storage-acceptance-tests','test_storage_acceptance.StorageAcceptanceTests')]:
        row = by_id[check_id]
        require(type(row.get('test_count')) is int and row['test_count'] > 0 and row['command'][-1] == suite, 'missing actual storage guard suite: ' + check_id)
    boundary_step = by_id['storage-boundary-execution']
    boundary_file = local_file(boundary_step.get('evidence_path'))
    require(hashlib.sha256(boundary_file.read_bytes()).hexdigest() == boundary_step.get('evidence_sha256'), 'boundary receipt hash mismatch')
    boundary = json.loads(boundary_file.read_text())
    registry = json.loads((root / 'docs/architecture/database/field-boundaries.json').read_text())
    registry_hash = hashlib.sha256(json.dumps(registry,ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()
    require(boundary.get('registry_sha256') == registry_hash, 'boundary registry identity mismatch')
    require(boundary.get('status') == 'passed' and boundary.get('errors') == [] and boundary.get('not_product_acceptance') is True, 'boundary execution failed or promoted to product')
    cases = boundary.get('checks', [])
    require(len(cases) == len(registry['probes']) and {item['id'] for item in cases} == {item['id'] for item in registry['probes']}, 'boundary probes missing or duplicated')
    require(all(item.get('status') == 'passed' and item.get('skipped') is False and type(item.get('assertions')) is int and item['assertions'] > 0 for item in cases), 'boundary probes skipped or failed')
    require(summary.get('storage_boundary_probes') == len(cases) == boundary_step.get('executed_probes'), 'boundary execution count mismatch')
    require(summary.get('storage_boundary_assertions') == sum(item['assertions'] for item in cases) == boundary_step.get('assertions'), 'boundary assertion count mismatch')
    require(summary.get('product_storage_acceptance') == 'blocked', 'planning report must not promote unavailable production storage adapter')
    total = sum(row.get('test_count') or 0 for row in steps)
    require(summary.get('all_engineering_guard_tests') == total, 'engineering total mismatch')
    return summary


def main():
    if OUTPUT.exists():
        raise SystemExit("Report directory exists; use a reviewed new revision instead of overwriting evidence.")
    proof_path = ROOT / '.build/quality/database-design-verification.json'
    validate_current_database_proof(ROOT, proof_path)
    OUTPUT.mkdir(parents=True)
    (ROOT / 'docs/architecture/engineering-directory-map.svg').write_text(directory_map_svg())
    references = OUTPUT / 'references'
    for relative in ['docs/product', 'docs/design/first-product-v1', 'docs/design/macos-material-case-v1', 'docs/design/archive', 'docs/architecture']:
        shutil.copytree(ROOT / relative, references / relative)
    # Architecture references include existing task and regression documents.
    shutil.copytree(ROOT / 'docs/development', references / 'docs/development')
    shutil.copy2(ROOT / 'LICENSE', references / 'LICENSE')
    shutil.copy2(ROOT / 'AGENTS.md', references / 'AGENTS.md')
    shutil.copytree(ROOT / 'openspec', references / 'openspec')
    shutil.copytree(ROOT / 'scripts/quality', references / 'scripts/quality', ignore=shutil.ignore_patterns('__pycache__'))
    shutil.copytree(ROOT / 'scripts/report', references / 'scripts/report', ignore=shutil.ignore_patterns('__pycache__'))
    for relative in ['package.json', 'package-lock.json', '.github/workflows/engineering-gates.yml']:
        target = references / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, target)
    header = '<header><div class="eyebrow">LUNA · ENGINEERING BASELINE · 2026.10.19</div><h1>从已确认的产品，确定研发方式。</h1><span class="status">设计与技术方案 · 未进入客户端研发</span><p class="lead">沿用当前 PRD 和高保真稿，统一首版功能、技术栈、模块Owner、组件复用、设计规范、文件结构、数据库字段与逐层测试准入。</p><div class="metrics"><div><b>12</b><span>首版功能范围</span></div><div><b>16</b><span>模块与主责</span></div><div><b>34</b><span>组件族与复用</span></div><div><b>B0—B5</b><span>资源冻结至正式包回归</span></div></div><nav><a href="#baseline">架构与技术决策</a><a href="#modules">模块职责</a><a href="#components">组件复用</a><a href="#layout">目录结构</a><a href="#decision">决策记录</a><a href="#visual">视觉依据</a><a href="references/docs/design/first-product-v1/index.html">高保真整稿</a></nav></header>'
    summary = '<div class="summary-grid"><section class="chapter"><h2>采用的主线</h2><p>Tauri 2 + React/TypeScript + Rust；Rust为业务权威，SQLite持久化，原生Hook与词法worker各自隔离。前端以无默认主题的交互组件和统一tokens承接磨砂风格。</p></section><section class="chapter"><h2>当前执行边界</h2><p>已接入OpenSpec、代码边界和实际编译门禁；产品未迁移。Core七回调、真实模型和内置资源仍须产品实测；门禁自测通过不提升首版产品状态。</p></section></div>'
    storage_section = storage_enforcement_section(ROOT)
    documents = {key: render_document(key, title, source) for key, title, source in DOCS}
    directory_map = '<section class="chapter" id="directory-map"><div class="section-title"><h2>目标目录结构图</h2><span class="status">目标规划 · 按切片落地</span></div><p>沿实线查看目录层级；各卡片标出子目录和职责。独立 Core 位于客户端目录树之外，通过锁定产物接入。窄屏可横向滚动查看。</p><div class="directory-viewport" role="region" aria-label="目标目录结构图，可横向滚动" tabindex="0"><img class="directory-map" src="references/docs/architecture/engineering-directory-map.svg" alt="客户端目标目录层级，以及独立 luna-hooks-core 仓库的构建产物接入关系"></div><div class="map-links"><a href="references/docs/architecture/engineering-directory-map.svg" download>下载目录结构图 SVG</a><a href="#layout">查看完整文件树与文件职责</a></div></section>'
    header = header.replace('<a href="#layout">目录结构</a>', '<a href="#directory-map">目录结构图</a><a href="#layout">完整文件树</a>')
    header = header.replace('<a href="#visual">视觉依据</a>', '<a href="#governance">迭代执行规则</a><a href="#visual">视觉依据</a>')
    header = header.replace('<a href="#governance">迭代执行规则</a>', '<a href="#governance">迭代执行规则</a><a href="#gates">可执行门禁与五类负例</a>')
    header = header.replace('<a href="#visual">视觉依据</a>', '<a href="#test-layers">测试路径图</a><a href="#test-plan">测试与上线规划</a><a href="#test-matrix">用例矩阵</a><a href="#visual">视觉依据</a>')
    test_graph = '<section class="chapter" id="test-layers"><h2>从底层到正式包的九层验证</h2><p>128个用例定义、153个执行组合，覆盖12项FP、16项AI配置要求及原97个首版验收来源；全部尚未执行。并行分支按垂直切片收齐，最终正式包不得混用不同RC。</p><div class="directory-viewport" role="region" aria-label="九层测试路径图，可横向滚动" tabindex="0"><img class="directory-map" src="references/docs/development/first-product-test-layers.svg" alt="规范、领域、协议、基础设施、应用、组件、正式包、四游戏及质量分发九层路径"></div><div class="map-links"><a href="references/docs/development/first-product-test-layers.svg" download>下载测试路径图</a><a href="references/docs/development/first-product-test-executions.csv" download>153项执行工作表</a><a href="references/docs/development/first-product-test-coverage.csv" download>需求追溯表</a><a href="references/docs/development/first-product-test-enablement.csv" download>12项测试设施任务</a></div></section>'
    proof = ''
    proof_source = ROOT / '.build/quality/final-verification.json'
    if proof_source.is_file():
        evidence = json.loads(proof_source.read_text())
        quality = OUTPUT / 'quality-qa'
        quality.mkdir()
        shutil.copy2(proof_source, quality / 'summary.json')
        for folder in ['five-scenarios', 'visual-adversarial']:
            shutil.copytree(ROOT / '.build/quality' / folder, quality / folder)
        rows = ''.join(f'<tr><td>{html.escape(row["category"])}</td><td>{row["positive_exit_code"]}</td><td>{row["violation_exit_code"]}</td><td>{"已拒绝" if row["rejected"] else "未通过"}</td></tr>' for row in evidence['five_categories'])
        proof = f'<section class="chapter" id="gate-results"><h2>历史五类故障注入：实际执行记录</h2><p>早期工程门禁正反例共 {evidence["guard_test_count"]} 项通过；TypeScript与Rust编译通过。完整产品发布门禁按预期拒绝，产品源码未改动。本表保留早期实际CLI退出码；本轮回执见数据库设计检查章节。</p><div class="table-wrap"><table><thead><tr><th>故障类别</th><th>符合规范：退出码</th><th>注入违规：退出码</th><th>结果</th></tr></thead><tbody>{rows}</tbody></table></div><p>视觉测试重放已确认材质案例，并注入错误配色检查差异；真实产品截图与运行adapter尚未接入，相关产品验收继续阻断。</p><div class="reference-grid"><figure><img loading="lazy" src="quality-qa/visual-adversarial/approved-source-render.png" alt="守卫测试：未改材质的对照截图"><figcaption>守卫对照：按确认案例源文件渲染。</figcaption></figure><figure><img loading="lazy" src="quality-qa/visual-adversarial/wrong-material.png" alt="守卫测试：注入错误粉色后被图像差异拒绝"><figcaption>故障注入：错误材质被像素差异检查拒绝。</figcaption></figure></div><a href="quality-qa/summary.json">机器摘要与证据索引</a></section>'
        header = header.replace('<a href="#gates">可执行门禁与五类负例</a>', '<a href="#gates">可执行门禁</a><a href="#gate-results">五类故障实测</a>')
    tables = ''.join(render_table(*entry) for entry in TABLES if entry[0] in {'modules', 'components'})
    database_tables = ''.join(render_table(*entry) for entry in TABLES if entry[0].startswith('database-'))
    test_table = render_table(*next(entry for entry in TABLES if entry[0] == 'test-matrix'))
    plan_proof = ROOT / '.build/quality/test-plan-verification.json'
    if plan_proof.is_file():
        quality = OUTPUT / 'quality-qa'
        quality.mkdir(exist_ok=True)
        shutil.copy2(plan_proof, quality / 'test-plan-verification.json')
        planning = json.loads(plan_proof.read_text())
        receipt_source = ROOT / planning['receipt_path']
        receipt_folder = quality / 'layered-plan-run'
        receipt_folder.mkdir()
        shutil.copy2(receipt_source, receipt_folder / 'receipt.json')
        for step in json.loads(receipt_source.read_text())['steps']:
            if 'log' in step:
                log_source = ROOT / step['log']
                shutil.copy2(log_source, receipt_folder / log_source.name)
        test_graph += f'<section class="chapter" id="test-plan-proof"><h2>前次测试规划检查记录</h2><p>前次计划守卫与导出正反例 {planning["planning_guard_tests"]} 项通过，登记与导出一致；TypeScript/Rust实际编译通过。检查只证明测试规划和工程设施，153个产品组合全部not_run，未运行游戏或模型。</p><a href="quality-qa/test-plan-verification.json">查看检查摘要与边界说明</a> · <a href="quality-qa/layered-plan-run/receipt.json">实际命令、退出码与日志索引</a></section>'
    # The database design receipt is current-run proof; prior guard receipts remain historical.
    database_artifacts = ['schema-contract.json', 'field-boundaries.json', 'boundary-fixtures.json', 'profile.sql', 'config.sql', 'dictionary.sql', 'schema-metadata.json', 'relationships.svg', 'field-dictionary.md']
    with (ROOT / 'docs/architecture/database/field-dictionary.csv').open(newline='') as stream:
        field_count = sum(1 for _ in csv.DictReader(stream))
    with (ROOT / 'docs/architecture/database/table-catalog.csv').open(newline='') as stream:
        table_count = sum(1 for _ in csv.DictReader(stream))
    db_links = ' · '.join(f'<a href="references/docs/architecture/database/{name}" download>{name}</a>' for name in database_artifacts)
    database_graph = f'<section class="chapter" id="database-relationships"><div class="section-title"><h2>数据库实体关系图</h2><span class="status">候选字段设计 · 待复核</span></div><p>{table_count} 张表、{field_count} 个字段，分为每配置档独立写库与随包只读辞书。图示主链和实体分组，完整复合外键、索引和触发器见字段字典及元数据；系统凭据库保持独立。</p><div class="directory-viewport" role="region" aria-label="数据库关系图，可横向滚动" tabindex="0"><img class="directory-map" src="references/docs/architecture/database/relationships.svg" alt="数据库主链、实体分组以及可写用户库、只读辞书和系统凭据边界"></div><div class="map-links">{db_links}</div></section>'
    header = header.replace('<a href="#modules">模块职责</a>', '<a href="#database-relationships">数据库关系图</a><a href="#database">存储设计</a><a href="#database-fields">逐字段字典</a><a href="#modules">模块职责</a>')
    header = header.replace('<a href="#test-matrix">用例矩阵</a>', '<a href="#test-matrix">用例矩阵</a><a href="#storage-enforcement">存储验收与边界</a><a href="#test-operations">测试供给与维护</a>')
    db_proof_source = ROOT / '.build/quality/database-design-verification.json'
    if not db_proof_source.is_file():
        raise ValueError('Current database design verification receipt is required before reporting checks')
    db_proof = validate_current_database_proof(ROOT, db_proof_source)
    quality = OUTPUT / 'quality-qa'
    quality.mkdir(exist_ok=True)
    shutil.copy2(db_proof_source, quality / 'database-design-verification.json')
    db_run = ROOT / db_proof['receipt_path']
    db_run_folder = quality / 'database-design-run'
    db_run_folder.mkdir()
    shutil.copy2(db_run, db_run_folder / 'receipt.json')
    for step in json.loads(db_run.read_text())['steps']:
        if 'log' in step:
            log_path = ROOT / step['log']
            shutil.copy2(log_path, db_run_folder / log_path.name)
    boundary_step = next(step for step in json.loads(db_run.read_text())['steps'] if step['id'] == 'storage-boundary-execution')
    boundary_evidence = ROOT / boundary_step['evidence_path']
    shutil.copy2(boundary_evidence, quality / 'field-boundary-execution.json')
    database_graph += f'<section class="chapter" id="database-proof"><h2>本轮数据库设计检查</h2><p>隔离SQLite设计测试 {db_proof["database_design_tests"]} 项通过，字段/关系导出一致；全量工程检查实际执行 {db_proof["all_engineering_guard_tests"]} 项，TypeScript与Rust编译通过。只在内存或临时库检验DDL，未安装迁移、访问用户库或实现产品数据库适配器。现有153项产品组合继续not_run。</p><a href="quality-qa/database-design-verification.json">本轮检查摘要与边界</a> · <a href="quality-qa/database-design-run/receipt.json">命令、退出码与日志索引</a></section>'
    images = [
        ('35-library-material-dark.png', '游戏库 · 连续暗蓝灰磨砂与统一图标'),
        ('23-connection-entities.png', '模型设置 · 连接、模型和用途分层'),
        ('09-hud-detail.png', '字幕 · 原文注音、译文和32pt控制行'),
        ('15-learning-result.png', '精读 · 固定对象与独立学习窗口'),
    ]
    visual = '<section class="chapter" id="visual"><h2>当前视觉基准与组件落点</h2><p>下图来自既有50张设计截图，用于解释组件/材质的复用，不是本轮新增产品运行证据。</p><div class="reference-grid">'
    for name, label in images:
        visual += f'<figure><a href="references/docs/design/first-product-v1/previews/{name}"><img loading="lazy" src="references/docs/design/first-product-v1/previews/{name}" alt="{html.escape(label)}"></a><figcaption>{html.escape(label)}</figcaption></figure>'
    visual += '</div></section>'
    script = """document.querySelectorAll('[data-filter]').forEach(input=>input.addEventListener('input',()=>{const table=document.getElementById(input.dataset.filter),rows=[...table.tBodies[0].rows],query=input.value.trim().toLocaleLowerCase();let visible=0;rows.forEach(row=>{row.hidden=!!query&&!row.textContent.toLocaleLowerCase().includes(query);if(!row.hidden)visible++});document.getElementById(input.dataset.filter.replace('-table','-count')).textContent=visible+' / '+rows.length+' 项'}));"""
    document = f'<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna · 首版研发架构、逐层测试与上线准入</title><style>{STYLE}</style></head><body>{header}<main>{summary}{documents["baseline"]}{tables}{database_graph}{storage_section}{documents["database"]}{documents["storage-acceptance"]}{database_tables}{directory_map}{documents["layout"]}{documents["decision"]}{documents["governance"]}{documents["gates"]}{proof}{test_graph}{documents["test-plan"]}{test_table}{documents["test-operations"]}{documents["iteration-template"]}{visual}</main><footer>架构方案与研发门禁报告；原始PRD、许可证和历史证据保留，产品回归仍为NOT_RUN。新增工程工具与实际编译检查不代表产品已按新架构完成。未修改产品源码，未启动游戏，未提交推送。<br><a href="report-manifest.json">源文件与包内哈希清单</a></footer><script>{script}</script></body></html>'
    (OUTPUT / 'index.html').write_text(document)
    sources = [entry[2] for entry in DOCS + TABLES] + ['docs/architecture/engineering-directory-map.svg', 'docs/development/iteration-baseline.json', 'AGENTS.md', 'docs/development/first-product-test-plan.json', 'docs/development/first-product-test-layers.svg'] + ['docs/architecture/database/' + name for name in database_artifacts]
    inventory = [{'path': str(p.relative_to(OUTPUT)), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': p.stat().st_size} for p in sorted(OUTPUT.rglob('*')) if p.is_file()]
    manifest = {'revision': 'first-product-engineering-v1.6-storage-enforcement', 'date': '2026-10-19', 'phase': 'design_and_engineering_tooling', 'source_sha256': {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in sources}, 'modules': 16, 'component_families': 34, 'test_definitions': 128, 'planned_executions': 153, 'test_layers': 9, 'storage_enforcement': {'fields': field_count, 'product_acceptance': 'blocked', 'boundary_probes': db_proof['storage_boundary_probes']}, 'database_tables': table_count, 'database_fields': field_count, 'database_design_approval': 'pending_review', 'baseline_id': json.loads((ROOT / 'docs/development/iteration-baseline.json').read_text())['baseline_id'], 'runtime_tests': 'NOT_RUN', 'product_code_changed': False, 'files': inventory}
    (OUTPUT / 'report-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    archive = OUTPUT.with_suffix('.zip')
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
        for path in sorted(OUTPUT.rglob('*')):
            if path.is_file(): z.write(path, OUTPUT.name + '/' + str(path.relative_to(OUTPUT)))
    print(json.dumps({'html': str((OUTPUT / 'index.html').relative_to(ROOT)), 'zip': str(archive.relative_to(ROOT)), 'bytes': archive.stat().st_size, 'sha256': hashlib.sha256(archive.read_bytes()).hexdigest()}, ensure_ascii=False))



def reviewed_report():
    """New immutable report edition; preserve old report and original references."""
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--reviewed-design', action='store_true')
    parser.add_argument('--output', required=True)
    parser.add_argument('--evidence', required=True)
    args = parser.parse_args()
    output = (ROOT / args.output).resolve()
    if not output.is_relative_to(ROOT / 'reports') or output.exists() or output.with_suffix('.zip').exists():
        raise ValueError('Use a fresh report directory under reports; never overwrite evidence')
    evidence_path = (ROOT / args.evidence).resolve()
    receipt = json.loads(evidence_path.read_text())
    if receipt.get('status') != 'passed' or receipt.get('profile') != 'development' or receipt.get('source_tree_sha256') != current_source_tree_hash(ROOT):
        raise ValueError('A fresh passed engineering receipt for current sources is required')
    if receipt.get('product_runtime_tests_executed') is not False:
        raise ValueError('This report edition must not claim product execution')
    rw_step = next(row for row in receipt['steps'] if row['id'] == 'interaction-design-execution')
    sys.path.insert(0, str(ROOT / 'scripts/quality'))
    from interaction_coverage import release_coverage_gaps, validate_registry
    if validate_registry(ROOT) or rw_step.get('status') != 'passed':
        raise ValueError('RW registry/actual design execution did not pass')
    from implementation_status import validate_catalog, CATALOG
    if validate_catalog(ROOT):
        raise ValueError('Current implementation status/catalog exports are inconsistent')
    if not any(row['id']=='implementation-status-contract' and row['status']=='passed' for row in receipt['steps']):
        raise ValueError('Current implementation status contract did not execute')
    catalog = json.loads((ROOT / CATALOG).read_text())
    gaps = release_coverage_gaps(ROOT, rw_step['execution'])
    if any(row['layer'] == 'design' for row in gaps):
        raise ValueError('RW current design receipt is not independently valid')
    output.mkdir(parents=True)
    refs = output / 'references'
    for relative in ['docs','openspec','scripts/quality','scripts/report']:
        shutil.copytree(ROOT / relative, refs / relative, ignore=shutil.ignore_patterns('__pycache__'))
    for relative in ['LICENSE','AGENTS.md','package.json','package-lock.json']:
        shutil.copy2(ROOT / relative, refs / relative)
    # Bundle only the audited text/manifest evidence, not whole product binaries/user data.
    evidence_paths = {entry['path'] for group in ('modules','components','capabilities') for row in catalog[group] for entry in row['source_evidence']}
    evidence_paths.update(entry['path'] for entry in catalog['historical_evidence'])
    evidence_paths.update(entry['path'] for entry in catalog.get('audit_inputs', []))
    evidence_paths.add('dist/build-manifest.json')
    for relative in sorted(evidence_paths):
        dest=refs/relative
        if dest.exists():
            continue
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT/relative, dest)
    # Keep all relative document links offline, including references to excluded historical run evidence.
    evidence = output / 'evidence'; evidence.mkdir()
    shutil.copy2(evidence_path, evidence / 'engineering-receipt.json')
    execution = rw_step['execution']
    rw_receipt = ROOT / execution['receipt']
    shutil.copytree(rw_receipt.parent, evidence / 'design-run')
    for row in receipt['steps']:
        if row.get('log') and (ROOT / row['log']).is_file():
            shutil.copy2(ROOT / row['log'], evidence / Path(row['log']).name)
    approval = json.loads((ROOT / 'docs/development/design-approval-20261019.json').read_text())
    registry = json.loads((ROOT / 'docs/development/reading-window-test-coverage.json').read_text())
    rows = registry['cases']
    with (ROOT / 'docs/architecture/engineering-components.csv').open(newline='') as stream:
        component_count = len(list(csv.DictReader(stream)))
    approved_images = list(approval['reviewed_screenshots_sha256'])
    # Render actual architecture source diagrams as source, alongside the correct graphical dataflow.
    extra_docs = [('implementation-status','当前实现、设计差距与证据范围','docs/architecture/implementation-status.md'),
                  ('reading-window','运行窗口、HUD与会话专项技术设计','docs/architecture/reading-window-session-technical-design.md'),
                  ('rw-tests','34项RW逐层测试合同','docs/development/reading-window-test-coverage.md'),
                  ('release-evidence','153个发布执行组合与证据','docs/development/release-execution-evidence.md')]
    doc_list = [entry for entry in DOCS if entry[0] != 'storage-acceptance'] + extra_docs
    chapters = ''.join(render_document(*entry,replace_mermaid=False) for entry in doc_list if entry[0]!='implementation-status')
    tables = ''.join(render_table(key, ('36 个组件族与复用' if key=='components' else title), source) for key,title,source in TABLES)
    layers=['design','product_component','real_pipeline','real_language','native','game']
    counts={layer:sum(row['layers'][layer]['applicable'] for row in rows) for layer in layers}
    gap_map={(row['case_id'],row['layer']):row for row in gaps}
    coverage_rows=''
    for row in rows:
        cells=''.join('<td>'+('不适用' if not row['layers'][layer]['applicable'] else ('passed · 设计' if layer=='design' and (row['id'],layer) not in gap_map else 'not_run · 缺真实adapter'))+'</td>' for layer in layers)
        coverage_rows+='<tr><th>'+html.escape(row['id'])+'</th>'+cells+'</tr>'
    coverage='<section class="chapter" id="rw-coverage"><h2>逐项逐层覆盖与当前结果</h2><p>34项权威验收，'+str(sum(counts.values()))+'个适用层组合。设计与产品真实能力独立；设计层32项已实际执行，其余'+str(len(gaps))+'个必需层组合缺真实adapter，发布拒绝。原153个case@variant仍须另行执行，不被这张矩阵代替。</p><div class="table-wrap"><table id="rw-coverage-table"><thead><tr><th>验收</th>'+''.join('<th>'+x+'</th>' for x in layers)+'</tr></thead><tbody>'+coverage_rows+'</tbody></table></div></section>'
    checks=json.loads(rw_receipt.read_text())['checks']
    proof='<section class="chapter" id="execution-proof"><h2>本轮实际验证与限制</h2><p>完整工程门禁 '+str(len(receipt['steps']))+' 项通过，含实际TypeScript/Rust编译和五类故障守卫。设计实际执行 '+str(len(checks))+' 项、'+str(sum(x['assertion_count'] for x in checks))+' 个断言；source/nonce/进程日志/截图双向关联均独立核对。设计批准不追认旧包或新产品能力。</p><p>源码核查已确认必须后续修复并实测的路径：暂停时raw持久化前返回；Hook阶段事件被忽略与100项内存诊断；活动会话GC；分词关闭连带ruby消失；原生Select及平台窗口动作缺完整验收。本轮没有修改产品业务。</p><p><a href="evidence/engineering-receipt.json">完整门禁回执</a> · <a href="evidence/design-run/verification.json">真实设计执行与截图索引</a> · <a href="references/docs/development/design-approval-20261019.json">用户批准身份</a></p></section>'
    graph=diagram().replace('React · 四个窗口','React · 四个窗口').replace('HookBackend','HookBackend · Core').replace('其他 I/O 端口','其他 I/O 端口 / WindowPreview')
    flow='<section class="chapter" id="dataflow"><h2>系统边界与新增数据流</h2><p>UI → application-port → Rust业务权威 → 窄端口。现有Core接入、raw保存和独立语言worker已有实现；预览帧受限通道、完整Hook日志与保留策略仍为待实施契约。各路径的实现及缺口以当前状态清单为准。下图为整体边界，专项章节保留完整时序与接口草图。</p>'+graph+'</section>'
    directory='<section class="chapter" id="directory-map"><h2>目录结构图与新增职责</h2><div class="directory-viewport" tabindex="0"><img class="directory-map" src="directory-map.svg" alt="客户端目录层级与独立hooks-core边界"></div><p>WindowPreview、SessionRecordPanel、WindowCoordinator等拟议文件和职责见目录组织与专项设计；未为图示创建空产品目录。</p></section>'
    (output/'directory-map.svg').write_text(directory_map_svg())
    nav=''.join('<a href="#'+key+'">'+title+'</a>' for key,title in [('implementation-status','当前实现与差距'),('execution-proof','验证与限制'),('dataflow','系统边界'),('reading-window','专项技术设计'),('directory-map','目录结构图'),('modules','模块职责'),('components','组件复用'),('database-fields','数据库字段'),('rw-coverage','覆盖矩阵'),('rw-tests','测试改进'),('release-evidence','发布证据')])
    header='<header><div class="eyebrow">LUNA · REVIEWED DESIGN / ENGINEERING · 2026.10.20</div><h1>技术方案、当前实现与设计差距</h1><span class="status">源码核对 2026-10-20 · 当前实机未重测 · 发布未通过</span><p class="lead">对齐最新预览、浮窗、Select、会话全量记录与词法呈现设计，保留Rust权威、窄Tauri适配和独立hooks-core边界。记录真实执行，拒绝缺层与跨层替代。</p><div class="metrics"><div><b>34</b><span>RW验收来源</span></div><div><b>16 / '+str(component_count)+'</b><span>模块 / 组件族</span></div><div><b>'+str(sum(counts.values()))+'</b><span>逐层必需组合</span></div><div><b>153</b><span>原发布执行组合</span></div></div><nav>'+nav+'<a href="references/docs/design/first-product-v1/index.html">已评审交互稿</a></nav></header>'
    script="""document.querySelectorAll('[data-filter]').forEach(input=>input.addEventListener('input',()=>{const table=document.getElementById(input.dataset.filter),rows=[...table.tBodies[0].rows],query=input.value.trim().toLocaleLowerCase();let visible=0;rows.forEach(row=>{row.hidden=!!query&&!row.textContent.toLocaleLowerCase().includes(query);if(!row.hidden)visible++});document.getElementById(input.dataset.filter.replace('-table','-count')).textContent=visible+' / '+rows.length+' 项'}));"""
    content=f'<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna 技术设计与测试改进</title><style>{STYLE}</style></head><body>{header}<main>{render_document('implementation-status','先看当前落地状态','docs/architecture/implementation-status.md',replace_mermaid=False)}{proof}{flow}{directory}{coverage}{chapters}{tables}</main><footer>本轮仅工程工具、规范与技术文档；无产品改动、无提交推送。批准原图、材质与数据库合同保持原字节。<a href="report-manifest.json">报告源与制品身份</a></footer><script>{script}</script></body></html>'
    # Report-local references that link excluded historic evidence are visibly marked, not fabricated.
    def rewrite_report_link(match):
        href=html.unescape(match.group(1))
        if href.startswith('#') or re.match(r'^[a-z]+:',href):return match.group(0)
        target=(output/unquote(urlsplit(href).path)).resolve()
        if target.is_file() or target.is_dir():return match.group(0)
        return 'data-source-reference="'+html.escape(href,quote=True)+'" title="源文档历史证据链接未随本包复制"'
    content=re.sub(r'href="([^"]+)"',rewrite_report_link,content)
    (output/'index.html').write_text(content)
    sources=[entry[2] for entry in doc_list+TABLES]+['docs/architecture/implementation-status.json','docs/development/reading-window-test-coverage.json','docs/development/design-approval-20261019.json','docs/development/iteration-baseline.json']
    inventory=[{'path':str(p.relative_to(output)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in sorted(output.rglob('*')) if p.is_file()]
    manifest={'schema_version':2,'revision':'reviewed-design-implementation-status-20261020','design_review':'approved','technical_increment':'existing_implementation_with_identified_design_gaps','product_code_changed':False,'product_tests_executed':False,'release_status':'blocked','rw_cases':len(rows),'applicable_layer_combinations':sum(counts.values()),'layer_counts':counts,'runtime_layer_gaps':len(gaps),'original_release_executions':153,'modules':16,'components':component_count,'engineering_steps':len(receipt['steps']),'implementation_catalog_sha256':hashlib.sha256((ROOT/CATALOG).read_bytes()).hexdigest(),'implementation_capabilities':len(catalog['capabilities']),'implementation_snapshot_date':catalog['audit_date'],'implementation_counts':{state:sum(row['implementation_status']==state for row in catalog['capabilities']) for state in ('implemented','partial','not_implemented')},'source_tree_sha256':receipt['source_tree_sha256'],'source_sha256':{p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources},'files':inventory}
    (output/'report-manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
    archive=output.with_suffix('.zip')
    with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED) as z:
        for p in sorted(output.rglob('*')):
            if p.is_file():z.write(p,output.name+'/'+str(p.relative_to(output)))
    print(json.dumps({'html':str((output/'index.html').relative_to(ROOT)),'zip':str(archive.relative_to(ROOT)),'sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'bytes':archive.stat().st_size},ensure_ascii=False))

if __name__ == '__main__':
    if '--reviewed-design' in sys.argv:
        reviewed_report()
    else:
        main()
