#!/usr/bin/env python3
"""Build a self-contained architecture HTML and a portable reference ZIP."""

import argparse
import base64
import csv
import hashlib
from html import escape
from html.parser import HTMLParser
import json
import mimetypes
from pathlib import Path
import re
import shutil
import sys
from urllib.parse import quote, unquote, urlsplit
import zipfile


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / ".build/report-tools"))
sys.path.insert(0, str(ROOT / "scripts/report"))

try:
    import markdown
except ImportError:
    raise SystemExit("Install report dependencies: python3 -m pip install --target .build/report-tools -r scripts/report/requirements.txt")

from diagrams import DIAGRAMS, thread_diagram
from check_project_plan import check


DOC = Path("docs/architecture/development-architecture.md")
ORIGINAL = Path("docs/product/original")
SMOKE = Path("diagnostics/headless-host-smoke-20261018-r3")
ASSETS = ROOT / "scripts/report"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reference(path):
    return "references/" + quote(str(path), safe="/")


def figure(title, body, caption, badge="离线图表"):
    return ('<figure class="figure"><div class="figure-heading"><h3>' + escape(title)
            + '</h3><span class="badge neutral">' + escape(badge) + '</span></div>'
            + body + '<figcaption>' + escape(caption) + '</figcaption></figure>')


def render_markdown():
    source = (ROOT / DOC).read_text()
    # The hero carries the document title and version; preserve the substantive introduction.
    source = re.sub(r"^# [^\n]+\n\n版本：[^\n]+\n", "", source, count=1)
    number = 0

    def replace_diagram(found):
        nonlocal number
        if number >= len(DIAGRAMS):
            raise ValueError("New Mermaid block needs an explicitly authored offline diagram")
        functions = DIAGRAMS[number]
        captions = [
            "已实现路径与无目标 smoke 范围。尚未 attach，图中的回调能力不等于真实游戏 Output 已验收。",
            "绿色：当前宿主或保留的 C++ 组件；蓝色虚线：规划模块。私有 JSONL 与 DLL 内部 C ABI 是两种边界。",
            "连接的逻辑阶段。helper 退出与异步 native 回调可能交错，不能将图中的排布解读为严格时序保证。",
            "规划中的翻译一致性约束。按会话、句子和修订归属结果，迟到结果不覆盖当前句。",
        ]
        titles = ["当前最小系统", "目标产品的组件与进程边界", "连接与原文回调时序", "迟到译文与当前句隔离"]
        result = figure(titles[number], functions(), captions[number])
        result += ('<details class="diagram-source"><summary>查看对应的 Mermaid 定义</summary><pre><code>'
                   + escape(found.group(1)) + '</code></pre></details>')
        number += 1
        return "\n\n" + result + "\n\n"

    source = re.sub(r"```mermaid\n(.*?)\n```", replace_diagram, source, flags=re.S)
    if number != len(DIAGRAMS):
        raise ValueError("Architecture Mermaid diagram count changed; review offline SVGs")
    slugify = lambda value, separator: re.sub(r"[^a-z0-9\u3400-\u9fff]+", separator, value.lower()).strip(separator)
    renderer = markdown.Markdown(extensions=["tables", "fenced_code", "toc", "sane_lists"],
                                 extension_configs={"toc": {"slugify": slugify, "toc_depth": "2-2"}})
    body = renderer.convert(source)
    body = rewrite_links(body)
    thread = figure("宿主线程、队列与控制状态", thread_diagram(),
                    "两条事件队列各 256；连接控制另存原子状态。公开响应和错误也可能丢失，不能据此承诺可靠交付。")
    body = body.replace('<h3 id="53-', thread + '<h3 id="53-', 1)
    # Heading slug retains its original numbering; use text as a fallback if naming is edited.
    if thread not in body:
        body = re.sub(r'(?=<h3[^>]*>5\.3 )', lambda _: thread, body, count=1)
    return body, renderer.toc


def rewrite_links(body):
    def replace(found):
        original_url = found.group(1)
        parts = urlsplit(original_url)
        if parts.scheme or original_url.startswith("#"):
            return found.group(0)
        target = (ROOT / DOC.parent / unquote(parts.path)).resolve()
        try:
            relative = target.relative_to(ROOT)
        except ValueError as error:
            raise ValueError(f"Document link escapes project: {original_url}") from error
        if not target.is_file():
            raise ValueError(f"Missing architecture reference: {relative}")
        url = "gallery.html" if relative == ORIGINAL / "design/index.html" else reference(relative)
        if parts.fragment:
            url += "#" + quote(parts.fragment)
        return 'href="' + escape(url, quote=True) + '"'
    return re.sub(r'href="([^"]+)"', replace, body)


def platform_figure():
    cards = [
        ("windows", "Windows 原生", "宿主实现；原生运行待验证", [
            ("Tauri / Rust 应用（规划）", "plan"),
            ("Windows x64 Rust 宿主 + Host64", "native"),
            ("目标 x86/x64 游戏 + Hook32/64", "native"),
        ], "PID、路径和 FILETIME 来自 Windows。Hook 与 helper 按目标位数选择。"),
        ("crossover", "macOS + CrossOver", "无目标初始化已验证", [
            ("macOS Tauri / Rust 应用（规划）", "plan"),
            ("指定 bottle：Windows x64 宿主", "native"),
            ("同 bottle：游戏 + 对应 Hook", "native"),
        ], "Windows 宿主仍是 PE 程序。macOS PID 不能替代 Windows 目标 PID。"),
        ("macos", "原生 macOS 游戏", "独立后端，尚待实施", [
            ("macOS Tauri / Rust 应用（规划）", "plan"),
            ("原生取字 / 进程身份适配（规划）", "plan"),
            ("真实原生游戏：逐目标验收", "plan"),
        ], "不能加载 Windows DLL 来实现原生取字。共享领域事件、翻译和 UI。"),
    ]
    body = '<div class="platform-tabs" aria-label="聚焦执行环境">'
    for value, label in [("all", "全部环境"), ("windows", "Windows"), ("crossover", "CrossOver"), ("macos", "原生 macOS")]:
        body += f'<button type="button" data-platform="{value}" aria-pressed="{str(value == "all").lower()}">{label}</button>'
    body += '</div><div class="platform-grid">'
    for value, title, status, layers, note in cards:
        body += f'<section class="platform-card" data-environment="{value}"><h4>{title}</h4><small>{status}</small><div class="environment">'
        for index, (label, kind) in enumerate(layers):
            if index:
                body += '<div class="arrow" aria-hidden="true">↓</div>'
            body += f'<div class="layer {kind}">{label}</div>'
        body += f'</div><p>{note}</p></section>'
    body += '</div>'
    return figure("三种环境的部署差异", body, "点击环境按钮可聚焦一条部署路线；打印时始终呈现全部环境。三条路线的业务模型共享，提取和系统能力分别验收。", "可切换")


def interface_appendix():
    commands = [
        ("ping", "{}", "存活响应 pong，不代表连接或取文"),
        ("status", "{}", "targets 阶段快照 + integrity_lost"),
        ("attach", "pid / expected_image_path / expected_creation_time", "核对身份后异步连接；返回 pending；此响应先于实际身份验证完成"),
        ("detach", "pid", "只针对回调已确认 connected 的目标，请求断开"),
        ("shutdown", "{}", "进入关闭阶段；2s 控制期限后最多300ms排空输出"),
    ]
    events = [
        ("ready", "host_arch / upstream_commit / abi_profile", "固定 Host 初始化完成"),
        ("response", "id / result", "请求处理结果；共用可丢输出队列"),
        ("error", "id? / code / message", "请求或宿主错误；不保证在堵塞时及时交付"),
        ("process_connected / process_disconnected", "pid", "native 进程回调；不等于引擎 prepared"),
        ("stream_created / stream_destroyed", "stream / hook_code / hook_name", "候选流生命周期；创建附 embed_capable"),
        ("text", "stream / hook_code / hook_name / text", "该流实际收到的新原文"),
        ("hook_inserted", "pid / address / hook_code", "native 报告 Hook 插入"),
        ("injection_finished", "pid / architecture / exit_code / accepted_by_helper", "原 helper 接受码1，不能据此宣称取文成功"),
        ("log", "kind / message", "Host 日志"),
        ("overflow", "source / dropped_since_last / dropped_total / integrity_lost", "已观察到丢失；不能重放丢失原文"),
    ]
    def table(headers, rows):
        return '<table><thead><tr>' + ''.join('<th>' + escape(item) + '</th>' for item in headers) + '</tr></thead><tbody>' + ''.join('<tr>' + ''.join('<td>' + escape(item) + '</td>' for item in row) + '</tr>' for row in rows) + '</tbody></table>'
    request = {"v": 1, "id": "health-1", "method": "ping", "params": {}}
    ready_lines = (ROOT / SMOKE / "stdout.jsonl").read_text()
    result = json.loads((ROOT / SMOKE / "result.json").read_text())
    if not result["passed"] or result["attached_games"] != 0 or result["scope"] != "initialization_and_commands_only":
        raise ValueError("Smoke evidence no longer matches this report's scope")
    errors = "unsupported_platform · invalid_request · duplicate_request · runtime_invalid · host_initialization · target_identity · unsupported_architecture · already_attached · not_attached · busy · injection_failed · connection_timeout · shutting_down · callback_invalid · shutdown_timeout · internal"
    return f'''<h2 id="interface-reference">16. 协议与当前参数速查</h2>
<p>以下内容描述当前 v1 实现；不包含第8～11章的应用层草案。方法共5种，事件枚举共12种（连接/断开、创建/销毁分别计数）。</p>
{table(["命令", "参数", "语义"], commands)}
<h3>16.1 事件与证据意义</h3>{table(["事件", "主要字段", "解释"], events)}
<p><code>stream = {{pid, address, context, context2}}</code>；后三项为十进制字符串。消费端用自己的环境和宿主实例包裹这个流身份。v1没有事件序号、重放或prepared事件。</p>
<h3>16.2 容量与期限</h3>
{table(["参数", "当前值", "限制"], [
    ("请求帧 / ID", "64 KiB / 1～128受限ASCII字符", "拒绝重复字段、未知字段、未知版本与方法"),
    ("回调队列 / 输出队列", "256 / 256", "满则计丢失；响应和错误也共用输出队列"),
    ("已尝试PID / attach在途", "16 / 1", "同宿主一个PID只尝试一次；重连使用新实例"),
    ("请求ID预算", "65,536", "预算耗尽仍允许新ID shutdown；历史ID不可复用"),
    ("连接 / helper / 关闭", "15s / 最多12s / 2s + 最多300ms输出排空", "不是覆盖全部初始化和系统调用的全局watchdog"),
    ("回调扫描上限", "65,536 UTF-16单元 / 4,096 hookname字节", "无效编码当前使用lossy转换；原始字节未保留"),
    ("Host历史", "每流10,000,000 UTF-16字符", "事件队列有界不能证明整个Host内存有界"),
])}
<h3>16.3 请求与真实无目标输出</h3><pre><code>{escape(json.dumps(request, ensure_ascii=False))}</code></pre>
<p><span class="badge">已验证</span> 下列四行为真实 smoke stdout；目标为空，没有attach。进程退出码0，stderr为空。原始文件和产物哈希保存在附件。</p><pre><code>{escape(ready_lines.rstrip())}</code></pre>
<h3>16.4 已定义错误码</h3><p class="report-note">{escape(errors)}</p>
<p>以上错误码均已定义，但各错误场景尚未全部完成运行验证；以 H2/H3 的逐场景记录判定。</p>'''


def requirement_appendix(rows):
    packages = sorted({item for row in rows for item in (row['work_package'] + ';' + row['related_work_packages']).split(';') if item})
    body = '''<h2 id="requirements">17. 完整需求与新任务映射</h2><p>逐项保留原 PRD 名称、优先级和全部页面关联。当前CSV中的56项产品需求均为 planned；工程基础检查通过不能直接把整个产品需求标为已交付。可按关键词、优先级或主/关联工作包筛选。</p>
<div class="filters"><label>关键词<input id="requirement-search" type="search" placeholder="需求编号、标题或页面编号"></label><label>优先级<select id="requirement-priority"><option value="">全部优先级</option><option>P0</option><option>P1</option><option>P2</option></select></label><label>工作包<select id="requirement-package"><option value="">全部工作包</option>'''
    body += ''.join(f'<option>{escape(package)}</option>' for package in packages)
    body += '</select></label><output id="requirement-count" aria-live="polite">显示 56 / 56 项</output></div>'
    body += '<table id="requirements-table" class="requirements-table"><thead><tr><th>编号</th><th>原需求</th><th>优先级</th><th>主责任</th><th>协作</th><th>设计页面</th></tr></thead><tbody>'
    for row in rows:
        members = row['work_package'] + ';' + row['related_work_packages']
        cells = [row['requirement_id'], row['title'], row['priority'], row['work_package'],
                 row['related_work_packages'].replace(';', ' / '), row['design_screens'].replace(';', ' / ')]
        body += f'<tr data-priority="{row["priority"]}" data-packages="{escape(members, quote=True)}">'
        body += ''.join(f'<td{" class=screens" if index == 5 else ""}>{escape(value)}</td>' for index, value in enumerate(cells)) + '</tr>'
    body += '</tbody></table><p id="requirement-empty" class="empty" hidden>没有符合条件的需求，可调整筛选条件。</p>'
    body += f'<p class="report-note">来源：<a href="{reference(Path("docs/development/requirements.csv"))}">requirements.csv</a>；关联集合由原PRD“页面与窗口归属”完整表校验。</p>'
    return body


def design_appendix(design):
    body = '''<h2 id="design-baseline">18. 原设计的继承与开发落点</h2><p>这里展示的是原设计资源，不是本次新客户端截图。89张图的既有视觉评审状态保留，运行功能仍按原稿记录为 NOT_RUN；新UI尚未实施。</p><div class="design-grid">'''
    for sid, key, caption in [
        ('SCR-01', 'ocr-reading-light', '阅读HUD：默认暂停与更多，隐藏和暂停分开'),
        ('SCR-04', 'light', '连接页面：明确显示目标、连接阶段与错误'),
        ('SCR-05', 'light', '候选文本流：用户明确选择正文与必要人名流'),
        ('SCR-15', 'light', '翻译服务：原设计对应的单层设置分类'),
    ]:
        screen = next(screen for screen in design['screens'] if screen['id'] == sid)
        preview = next(preview for preview in screen['previews'] if preview['key'] == key)
        path = ORIGINAL / 'design' / preview['file']
        data = (ROOT / path).read_bytes()
        uri = 'data:' + mimetypes.guess_type(path.name)[0] + ';base64,' + base64.b64encode(data).decode()
        url = 'gallery.html?screen=' + sid + '&amp;variant=' + quote(key)
        body += f'<figure class="design-card"><a href="{url}" aria-label="查看{sid}原设计"><img src="{uri}" alt="{escape(sid + " " + screen["title"] + " 原设计预览", quote=True)}" loading="lazy"></a><figcaption><strong>{sid} · {escape(screen["title"])}</strong><br>{escape(caption)}<small>原稿参考 · 非新工程运行结果</small></figcaption></figure>'
    body += '</div><p><a class="button" href="gallery.html">打开完整离线画廊：22屏 / 89图 ↗</a></p><p class="report-note">原始画廊依赖fetch读取manifest，直接file打开存在限制。本包另生成内置manifest的离线画廊；原始100文件仍逐字节保留。</p>'
    return body


def sources_appendix():
    items = [
        ('完整开发架构源稿', DOC),
        ('ADR-001：原生边界决策', Path('docs/architecture/ADR-001-headless-lunahook.md')),
        ('Hook API v1', Path('docs/architecture/hook-api-v1.md')),
        ('重新规划的17工作包', Path('docs/development/plan.md')),
        ('本轮验证记录', Path('docs/development/verification.md')),
        ('原始PRD v1.2', ORIGINAL / 'docs/01-prd.md'),
        ('原始设计规格', ORIGINAL / 'docs/02-design-spec.md'),
        ('原稿100文件来源清单', Path('docs/product/source-manifest.json')),
        ('原生运行时10文件锁', Path('native/upstream.lock.json')),
        ('真实smoke结果', SMOKE / 'result.json'),
        ('当前Rust协议源代码', Path('crates/hook-contracts/src/lib.rs')),
        ('当前Windows宿主源代码', Path('crates/lunahook-host/src/windows/mod.rs')),
    ]
    body = '<h2 id="source-package">19. 交付包、来源与使用方式</h2><p>解压ZIP后打开index.html即可阅读全文；原设计见gallery.html。正文图表、样式、交互与四张参考图均内置，不调用CDN、字体服务或外部API。外网仅用于主动点击上游源码链接。</p><div class="source-list">'
    for label, path in items:
        body += f'<a class="source-link" href="{reference(path)}">{escape(label)}<span>{escape(str(path))}</span></a>'
    body += '</div><p>附带当前两crate源代码和相关脚本用于审阅，不含旧Rust实现、旧任务、游戏文件、官方运行二进制或原版Luna私有配置。新宿主的无目标smoke证据单独选入，未打包旧对白或整个diagnostics目录。</p>'
    body += '<p>原稿中的历史开发门禁和未导入的旧文档引用保留作追溯，当前执行顺序以本次新架构与17工作包为准。原始文件的内容和既有视觉状态不因生成报告而改变。</p>'
    body += '<p class="report-note">package-manifest.json记录包内文件的大小和SHA-256，SHA256SUMS.txt可校验附件。QA截图与验证摘要位于包旁的qa目录，不进入本次代码/原稿事实判断。</p>'
    return body


def wrap_chapters(body):
    parts = re.split(r'(?=<h2 id=")', body)
    result = parts[0]
    for part in parts[1:]:
        identifier = re.match(r'<h2 id="([^"]+)"', part).group(1)
        result += f'<section class="chapter" id="{identifier}">' + re.sub(r' id="[^"]+"', '', part, count=1) + '</section>'
    return result.replace('<table', '<div class="table-wrap"><table').replace('</table>', '</table></div>')


def create_gallery(output, design):
    safe_json = json.dumps(design, ensure_ascii=False).replace('<', '\\u003c')
    document = '''<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna 原设计 · 离线画廊</title><style>
body{font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;margin:0;background:#f1f4f7;color:#172b3a;line-height:1.8}header{padding:22px 30px;background:white;border-bottom:1px solid #dce5e9}header h1{margin:4px 0;font-size:24px}header p{font-size:12px;color:#526875;margin:5px 0}a{color:#116e6b}.layout{display:grid;grid-template-columns:250px minmax(0,1fr);gap:24px;padding:25px 30px}aside{background:white;border-radius:9px;padding:15px;align-self:start}button,input{font:inherit}aside input{width:100%;box-sizing:border-box;padding:9px;border:1px solid #dce5e9;border-radius:6px;font-size:12px}nav{display:grid;gap:5px;margin-top:15px}button{padding:8px 10px;border:1px solid #dce5e9;border-radius:5px;background:white;color:#172b3a;cursor:pointer;text-align:left;font-size:12px}button[aria-pressed=true]{background:#e7f4f1;border-color:#116e6b;color:#116e6b}main{min-width:0}h2{font-size:21px;margin:0 0 6px}#status,#description{font-size:12px;color:#526875}#variants{display:flex;flex-wrap:wrap;gap:6px;margin:17px 0}figure{background:white;padding:18px;border:1px solid #dce5e9;border-radius:8px;margin:0}img{max-width:100%;max-height:78vh;height:auto;display:block;margin:auto}figcaption{font-size:12px;color:#526875;margin-top:15px;overflow-wrap:anywhere}button:focus-visible,input:focus-visible{outline:3px solid #2a91be;outline-offset:3px}@media(max-width:760px){.layout{grid-template-columns:1fr;padding:17px}nav{grid-template-columns:repeat(2,minmax(0,1fr));max-height:250px;overflow:auto}header{padding:17px}figure{padding:10px}}
</style></head><body><header><a href="index.html">← 返回开发架构报告</a><h1>原设计 · 22屏 / 89图</h1><p>内置manifest，双击即可离线阅读。以下是原设计资源，非新客户端运行截图；既有视觉状态不提升为功能验收。</p></header><div class="layout"><aside><input id="filter" type="search" aria-label="搜索设计页面" placeholder="页面编号或名称"><nav id="screens" aria-label="设计页面"></nav></aside><main><h2 id="title"></h2><p id="status"></p><div id="variants" aria-label="设计状态"></div><figure><a id="original"><img id="preview" alt=""></a><figcaption id="description"></figcaption></figure></main></div><script>
const manifest=@@MANIFEST@@;
const params=new URLSearchParams(location.search);let active=manifest.screens.find(s=>s.id===params.get('screen'))||manifest.screens[0];let variant=params.get('variant');
function renderNav(){const nav=document.querySelector('#screens');nav.replaceChildren();const q=document.querySelector('#filter').value.trim().toLowerCase();manifest.screens.filter(s=>(s.id+' '+s.title).toLowerCase().includes(q)).forEach(s=>{const b=document.createElement('button');b.textContent=s.id+' · '+s.title;b.setAttribute('aria-pressed',String(s===active));b.onclick=()=>{active=s;variant=null;render();};nav.append(b);});}
function render(){renderNav();document.querySelector('#title').textContent=active.id+' · '+active.title;document.querySelector('#status').textContent=active.status||'原设计状态未记录';const choice=active.previews.find(p=>p.key===variant)||active.previews.find(p=>p.key===active.reviewDefault)||active.previews[0];variant=choice.key;const variants=document.querySelector('#variants');variants.replaceChildren();active.previews.forEach(p=>{const b=document.createElement('button');b.textContent=p.title||p.key;b.setAttribute('aria-pressed',String(p===choice));b.onclick=()=>{variant=p.key;render();};variants.append(b);});const url='references/docs/product/original/design/'+choice.file;const img=document.querySelector('#preview');img.src=url;img.alt=active.id+' '+active.title+' / '+(choice.title||choice.key);document.querySelector('#original').href=url;document.querySelector('#description').textContent=(choice.description||'')+'　SHA-256: '+choice.sha256;history.replaceState(null,'','?screen='+encodeURIComponent(active.id)+'&variant='+encodeURIComponent(choice.key));}document.querySelector('#filter').addEventListener('input',renderNav);render();
</script></body></html>'''
    (output / 'gallery.html').write_text(document.replace('@@MANIFEST@@', safe_json))


def collect_references():
    manifest = json.loads((ROOT / 'docs/product/source-manifest.json').read_text())
    paths = set()
    for item in manifest['files']:
        relative = Path('docs/product') / item['localPath']
        if '..' in relative.parts or not relative.is_relative_to(ORIGINAL):
            raise ValueError(f'Product reference outside original snapshot: {relative}')
        paths.add(relative)
    # Reviewable allowlist: do not recursively sweep developer folders into a report.
    paths.update(Path(name) for name in (
        'Cargo.toml', 'Cargo.lock',
        'docs/architecture/development-architecture.md',
        'docs/architecture/ADR-001-headless-lunahook.md', 'docs/architecture/hook-api-v1.md',
        'docs/development/plan.md', 'docs/development/requirements.csv', 'docs/development/verification.md',
        'docs/product/README.md', 'docs/product/source-manifest.json',
        'native/README.md', 'native/upstream.lock.json',
        'crates/hook-contracts/Cargo.toml', 'crates/hook-contracts/src/lib.rs',
        'crates/lunahook-host/Cargo.toml', 'crates/lunahook-host/src/lib.rs',
        'crates/lunahook-host/src/main.rs', 'crates/lunahook-host/src/abi.rs',
        'crates/lunahook-host/src/framing.rs', 'crates/lunahook-host/src/queue.rs',
        'crates/lunahook-host/src/runtime.rs', 'crates/lunahook-host/src/windows/mod.rs',
        'crates/lunahook-host/src/windows/native.rs', 'crates/lunahook-host/src/windows/identity.rs',
        'crates/lunahook-host/src/windows/callbacks.rs',
        'scripts/build_architecture_report.py', 'scripts/check_project_plan.py',
        'scripts/prepare_lunahook_runtime.py', 'scripts/smoke_headless_host.py',
        'scripts/report/requirements.txt', 'scripts/report/diagrams.py',
        'scripts/report/report.html', 'scripts/report/report.css', 'scripts/report/report.js',
        'scripts/report/verify_report.cjs', 'tests/test_prepare_lunahook_runtime.py',
    ))
    paths.update(SMOKE / name for name in ('requests.jsonl', 'stdout.jsonl', 'stderr.txt', 'result.json', 'build-metadata.json'))
    for path in paths:
        absolute = ROOT / path
        if not absolute.resolve().is_relative_to(ROOT):
            raise ValueError(f'Report reference escapes project: {path}')
        if not absolute.is_file() or any(parent.is_symlink() for parent in [absolute, *absolute.parents] if parent.is_relative_to(ROOT)):
            raise ValueError(f'Nonregular or symbolic report reference: {path}')
        if path.suffix.lower() in ('.exe', '.dll', '.db', '.sqlite', '.zip'):
            raise ValueError(f'Unexpected binary/private data in report references: {path}')
    return sorted(paths)


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.urls = []
        self.ids = []

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name in ('href', 'src') and value:
                self.urls.append(value)
            if name == 'id':
                self.ids.append(value)


def validate_html(output):
    checked = 0
    for path in (output / 'index.html', output / 'gallery.html'):
        parser = Links()
        parser.feed(path.read_text())
        if len(set(parser.ids)) != len(parser.ids):
            raise ValueError(f'Duplicate HTML id in {path.name}')
        for url in parser.urls:
            parts = urlsplit(url)
            if parts.scheme or url.startswith('?'):
                continue
            if parts.path:
                target = path.parent / unquote(parts.path)
                if not target.is_file():
                    raise ValueError(f'Missing packaged HTML reference: {url}')
            elif parts.fragment and unquote(parts.fragment) not in parser.ids:
                raise ValueError(f'Missing HTML anchor: {url}')
            checked += 1
    return checked


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'reports/luna-development-architecture-20261018')
    args = parser.parse_args()
    output = args.output.resolve()
    archive = output.with_suffix('.zip')
    if output.exists() or archive.exists():
        raise ValueError('Choose a new output directory and ZIP path; existing report is preserved')
    product_check = check(ROOT)
    rows = list(csv.DictReader((ROOT / 'docs/development/requirements.csv').open(newline='')))
    design = json.loads((ROOT / ORIGINAL / 'design/manifest.json').read_text())
    body, toc = render_markdown()
    body = re.sub(r'(?=<h3[^>]*>4\.1 )', lambda _: platform_figure(), body, count=1)
    body += interface_appendix() + requirement_appendix(rows) + design_appendix(design) + sources_appendix()
    body = wrap_chapters(body)
    extra_links = ''.join(f'<li><a href="#{identifier}">{label}</a></li>' for identifier, label in [
        ('interface-reference', '16. 协议与参数速查'), ('requirements', '17. 完整需求映射'),
        ('design-baseline', '18. 原设计继承'), ('source-package', '19. 来源与交付包')])
    toc = toc.replace('</ul>', extra_links + '</ul>', 1)
    document = (ASSETS / 'report.html').read_text()
    for key, value in {'STYLE': (ASSETS / 'report.css').read_text(), 'SCRIPT': (ASSETS / 'report.js').read_text(), 'TOC': toc, 'BODY': body}.items():
        document = document.replace('@@' + key + '@@', value)
    output.mkdir(parents=True)
    (output / 'index.html').write_text(document)
    create_gallery(output, design)
    for path in collect_references():
        destination = output / 'references' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, destination)
    (output / 'READ-ME.txt').write_text('Luna Translator 开发架构报告 v1.0\n\n打开 index.html 阅读报告；gallery.html 为完整离线设计画廊。\n正文、图表、样式与交互无需联网。附录仅包含资料、当前代码和无目标验证证据，不包含可执行运行时。\n原设计与现有实现/规划在报告中明确区分，新宿主真实游戏取文尚待H2。\n\n重新生成（项目根目录；包内references为本次资料/代码快照）：\npython3 -m pip install --target .build/report-tools -r scripts/report/requirements.txt\npython3 scripts/build_architecture_report.py --output <新目录>\n\n报告离线QA（需本机Chrome，独立临时浏览器，不操作游戏）：\nnpm install --prefix .build/report-browser --no-save --package-lock=false playwright-core@1.56.1\nnode scripts/report/verify_report.cjs <报告目录> <新的QA输出目录>\n非默认Chrome路径可用REPORT_BROWSER_EXECUTABLE指定。生成器和QA均拒绝覆盖已有输出目录。\n')
    (output / 'package-manifest.json').write_text('{}')
    (output / 'SHA256SUMS.txt').write_text('')
    checked = validate_html(output)
    files = [{"path": str(path.relative_to(output)), "bytes": path.stat().st_size, "sha256": digest(path)}
             for path in sorted(output.rglob('*')) if path.is_file() and path.name not in ('package-manifest.json', 'SHA256SUMS.txt')]
    inventory = {'schema_version': 1, 'report_version': '1.0', 'date': '2026-10-18',
                 'product_validation': product_check, 'html_local_links_checked': checked,
                 'scope': 'report_and_reference_snapshot_without_runtime_binaries',
                 'source_document_sha256': digest(ROOT / DOC),
                 'files': files, 'manifest_exclusions': ['package-manifest.json', 'SHA256SUMS.txt']}
    (output / 'package-manifest.json').write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + '\n')
    checksum_files = files + [{'path': 'package-manifest.json', 'sha256': digest(output / 'package-manifest.json')}]
    (output / 'SHA256SUMS.txt').write_text(''.join(f'{item["sha256"]}  {item["path"]}\n' for item in checksum_files))
    with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as bundle:
        for path in sorted(output.rglob('*')):
            if path.is_file():
                bundle.write(path, str(Path(output.name) / path.relative_to(output)))
    with zipfile.ZipFile(archive) as bundle:
        if bundle.testzip() is not None:
            raise ValueError('ZIP checksum verification failed')
    print(json.dumps({'html': str(output / 'index.html'), 'zip': str(archive),
                      'zip_sha256': digest(archive), 'zip_bytes': archive.stat().st_size,
                      'packaged_files': len(files) + 2, 'local_links_checked': checked}, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError) as error:
        print(f'Architecture report failed: {error}', file=sys.stderr)
        sys.exit(1)
