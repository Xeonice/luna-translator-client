#!/usr/bin/env python3
"""Export the client rebuild plan as an offline report and reference ZIP."""

import argparse
import csv
import hashlib
from html import escape
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import quote, unquote, urlsplit
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CORE_COMMIT = "e9242ebac32a09e911f306b7c90714891c217302"
sys.path.insert(0, str(ROOT / ".build/report-tools"))
try:
    import markdown
except ImportError:
    raise SystemExit("Install the existing report dependencies in .build/report-tools first.")

from check_client_rebuild_plan import check

DOCS = [
    ("architecture", "重建架构", Path("docs/architecture/client-rebuild-architecture.md")),
    ("decision", "架构决策", Path("docs/architecture/ADR-003-client-rebuild-on-core.md")),
    ("plan", "实施顺序", Path("docs/development/plan.md")),
    ("regression", "产品回归", Path("docs/development/client-regression.md")),
    ("learning", "日语学习与大模型", Path("docs/architecture/japanese-study-and-llm.md")),
]
TABLES = [
    ("tasks", "开发任务", Path("docs/development/client-rebuild-tasks.csv")),
    ("cases", "回归用例", Path("docs/development/client-regression.csv")),
    ("requirements", "原需求映射", Path("docs/development/requirements.csv")),
    ("extensions", "学习能力补充", Path("docs/development/client-learning-extensions.csv")),
]


def architecture_svg():
    """An authored offline diagram of the planned process boundaries."""
    boxes = [
        (20, 20, "Tauri / React", "原设计 HUD、设置与历史"),
        (290, 20, "Rust 应用核心", "选流、阅读单元、翻译与配置"),
        (560, 20, "hook-client", "bottle、身份与会话监督"),
        (560, 190, "Windows Rust broker", "目标同位数 32 / 64"),
        (290, 190, "LunaHostCore", "Core ABI 1 · 七回调"),
        (20, 190, "游戏 + LunaHookCore", "同位数 Injector 负责加载"),
    ]
    parts = ['<svg role="img" aria-labelledby="arch-title arch-desc" viewBox="0 0 820 300" xmlns="http://www.w3.org/2000/svg">',
             '<title id="arch-title">计划中的客户端、broker 与 Core</title>',
             '<desc id="arch-desc">原生客户端经应用核心和环境管理，通过私有 JSONL 与 Windows broker 通信，broker 使用七回调 Host 取得游戏中的 Hook 原文。此迁移尚未实施。</desc>',
             '<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="#50627d"/></marker></defs>',
             '<g fill="none" stroke="#50627d" stroke-width="2" marker-start="url(#arrow)" marker-end="url(#arrow)"><path d="M255 65H285"/><path d="M525 65H555"/><path d="M680 110V185"/><path d="M560 235H530"/><path d="M290 235H260"/></g>',
             '<text x="685" y="153" class="edge">私有 JSONL v2</text>']
    for x, y, title, detail in boxes:
        parts.append(f'<rect x="{x}" y="{y}" width="235" height="90" rx="12" fill="#f0f5ff" stroke="#c5d5ee"/>')
        parts.append(f'<text x="{x+117.5}" y="{y+36}" text-anchor="middle" class="box-title">{escape(title)}</text>')
        parts.append(f'<text x="{x+117.5}" y="{y+62}" text-anchor="middle" class="box-detail">{escape(detail)}</text>')
    return "".join(parts) + '</svg>'


def render_doc(path, section_id):
    source = (ROOT / path).read_text()
    source = re.sub(r"^# .*?\n", "", source, count=1)
    source = re.sub(r"```mermaid\n.*?\n```", architecture_svg(), source, flags=re.S)
    body = markdown.markdown(source, extensions=["tables", "fenced_code", "sane_lists"])

    def link(match):
        url = match.group(1)
        parts = urlsplit(url)
        if parts.scheme or parts.netloc or url.startswith("#"):
            return match.group(0)
        absolute = (ROOT / path.parent / unquote(parts.path)).resolve()
        if absolute == ROOT / "docs/development/verification.md":
            return 'href="#baseline-notes"'
        if absolute == ROOT / "docs/product/original/design/index.html":
            return 'href="gallery.html"'
        if absolute.is_relative_to(ROOT):
            relative = absolute.relative_to(ROOT)
            destination = "references/" + quote(str(relative), safe="/")
        elif absolute.is_relative_to(ROOT.parent / "luna-hooks-core"):
            relative = absolute.relative_to(ROOT.parent / "luna-hooks-core")
            destination = "references/core/" + quote(str(relative), safe="/")
        else:
            raise ValueError(f"Unpackaged report link in {path}: {url}")
        if parts.fragment:
            destination += "#" + parts.fragment
        return f'href="{destination}"'

    body = re.sub(r'href="([^"]+)"', link, body)
    return f'<section id="{section_id}" class="chapter">{body}</section>'


def render_gallery(output):
    manifest = json.loads((ROOT / "docs/product/original/design/manifest.json").read_text())
    sections = []
    for screen in manifest["screens"]:
        previews = []
        for preview in screen["previews"]:
            src = "references/docs/product/original/design/" + quote(preview["file"], safe="/")
            label = preview.get("title") or preview["file"]
            metadata = " · ".join(str(preview[key]) for key in ["revision", "designAcceptance", "runtimeTests"] if key in preview)
            if metadata:
                label += " · " + metadata
            previews.append(f'<figure><img loading="lazy" src="{src}" alt="{escape(str(label))}" style="width:100%;height:auto"/><figcaption>{escape(str(label))}</figcaption></figure>')
        title = screen["id"] + " " + str(screen.get("title", screen.get("name", "")))
        status = escape(str(screen.get("status", "确认状态见原稿记录")))
        sections.append(f'<section class="chapter"><h2>{escape(title)}</h2><p>{status}</p>{"".join(previews)}</section>')
    html = ('<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
            f'<title>原设计离线画廊</title><style>{STYLE}</style><body><header><h1>原设计离线画廊</h1>'
            '<p>原始图片保持字节不变；本画廊内嵌索引，不需要服务器。确认状态以原稿记录为准，未确认稿不因展示而被视为已确认。</p>'
            '<a href="index.html">返回重建规划</a></header><main>' + "".join(sections) + '</main></body></html>')
    (output / "gallery.html").write_text(html)


def render_csv(section_id, title, path):
    with (ROOT / path).open(newline="") as stream:
        reader = csv.DictReader(stream)
        fields, rows = reader.fieldnames, list(reader)
    headings = "".join(f"<th>{escape(field)}</th>" for field in fields)
    body = "".join("<tr>" + "".join(f"<td>{escape(row[field])}</td>" for field in fields) + "</tr>" for row in rows)
    return (f'<section id="{section_id}" class="chapter"><h2>{title}</h2>'
            f'<p>{len(rows)} 条。空证据列表示尚无本轮实施或回归结果。'
            f'<a href="references/{path}" download>下载 CSV</a></p>'
            f'<label>筛选本表 <input type="search" data-table="{section_id}-table" placeholder="输入任务、REQ、游戏或状态"/></label>'
            f'<div class="table-wrap"><table id="{section_id}-table"><thead><tr>{headings}</tr></thead><tbody>{body}</tbody></table></div></section>')


STYLE = """
:root{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;color:#20314c;background:#eef2f8;font-size:16px;line-height:1.75}*{box-sizing:border-box}body{margin:0}a{color:#265ec6;text-underline-offset:3px}header,main,footer{max-width:1280px;margin:auto;padding:32px}header{padding-top:52px}header h1{font-size:36px;line-height:1.25;margin:12px 0}.eyebrow{color:#37649c;letter-spacing:.1em;font-size:13px}.scope{padding:14px 18px;border-left:4px solid #cf971e;background:#fff5d8}.stats{display:flex;gap:12px;flex-wrap:wrap;margin:24px 0}.stats div{background:white;border:1px solid #d7e0ec;border-radius:10px;padding:16px 24px;min-width:150px}.stats strong{font-size:27px;display:block}nav{display:flex;gap:10px 20px;flex-wrap:wrap}main{padding-top:0}.chapter{background:white;border:1px solid #d7e0ec;border-radius:14px;padding:28px 32px;margin-bottom:24px;overflow:hidden;scroll-margin-top:12px}h2{font-size:24px;line-height:1.4;margin-top:30px}h3{font-size:19px}p,li{overflow-wrap:anywhere}table{border-collapse:collapse;font-size:14px;width:100%;line-height:1.6}th,td{padding:10px 12px;border:1px solid #d9e1ed;text-align:left;vertical-align:top}th{background:#f0f4fa}td{min-width:140px;max-width:450px;overflow-wrap:anywhere}.table-wrap{overflow:auto;max-width:100%;border-radius:7px;margin-top:16px}input{font:inherit;border:1px solid #b8c8dd;border-radius:6px;padding:7px 10px;min-width:280px;max-width:100%}code{background:#f1f4f8;border-radius:3px;padding:1px 4px;font-size:.9em;overflow-wrap:anywhere}pre{background:#f1f4f8;padding:15px;overflow:auto}pre code{padding:0}blockquote{border-left:3px solid #bccbe1;margin-left:0;padding-left:16px;color:#50627d}svg{width:100%;height:auto;max-width:100%;margin:18px 0}svg text{font-family:-apple-system,BlinkMacSystemFont,sans-serif;fill:#20314c}.box-title{font-size:16px;font-weight:600}.box-detail{font-size:12px}.edge{font-size:12px}footer{font-size:14px;color:#5c6e86}.chapter>table{display:block;overflow:auto}tr[hidden]{display:none}@media(max-width:700px){header,main,footer{padding:20px}header h1{font-size:28px}.chapter{padding:18px 16px}.stats div{min-width:110px;padding:12px 16px}table{font-size:13px}input{min-width:0;width:100%}}@media print{body{background:white}header,main{max-width:none;padding:0}.chapter{border:0;padding:0;overflow:visible}nav,input,label{display:none}.table-wrap{overflow:visible}table{font-size:9px}td{min-width:0;max-width:none}tr[hidden]{display:table-row}a{color:inherit}h2{break-after:avoid}}
"""


def build(output):
    validation = check()
    if output.is_relative_to(ROOT / "docs/product/original"):
        raise ValueError("Report output cannot be inside the original product source tree.")
    archive = output.with_suffix(".zip")
    if output.exists() or archive.exists():
        raise ValueError("Refusing to overwrite an existing report or ZIP; choose --output.")
    core_files = {}
    for path in ["docs/abi.md", "verification/published-summary.json", "LICENSE", "THIRD_PARTY_NOTICES.md", "MODIFICATIONS.md"]:
        result = subprocess.run(["git", "-C", str(ROOT.parent / "luna-hooks-core"), "show", f"{CORE_COMMIT}:{path}"], capture_output=True)
        if result.returncode:
            raise ValueError(f"Cannot read fixed Core reference {path} at {CORE_COMMIT}.")
        core_files[path] = result.stdout
    output.mkdir(parents=True)
    refs = output / "references"
    docs = [path for _, _, path in DOCS + TABLES]
    docs += [Path("docs/development/desktop-client.md"),
             Path("docs/architecture/development-architecture.md"), Path("docs/architecture/ADR-002-source-trimmed-hooks.md"),
             Path("docs/architecture/ADR-001-headless-lunahook.md"), Path("docs/architecture/hook-api-v1.md"),
             Path("LICENSE")]
    for path in docs:
        target = refs / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, target)
    shutil.copytree(ROOT / "docs/product/original", refs / "docs/product/original")
    shutil.copytree(ROOT / "docs/design/first-product-v1", refs / "docs/design/first-product-v1")
    shutil.copytree(ROOT / "docs/design/macos-material-case-v1", refs / "docs/design/macos-material-case-v1")
    original_manifest = json.loads((ROOT / "docs/product/source-manifest.json").read_text())
    public_manifest = {"scope": "source metadata redacted for report; original file bytes unchanged",
                       "files": [{key: record[key] for key in ["localPath", "bytes", "sha256"]} for record in original_manifest["files"]]}
    (refs / "docs/product/source-manifest.json").write_text(json.dumps(public_manifest, ensure_ascii=False, indent=2) + "\n")
    (refs / "docs/product/README.md").write_text("# 原产品依据\n\n原 PRD、设计规范和全部 100 个原始文件保持字节不变。来源清单在本报告中去除远端主机与个人绝对路径，仅保留相对路径、大小和哈希。确认状态以原稿为准。\n")
    render_gallery(output)
    for path, data in core_files.items():
        target = refs / "core" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    nav = "".join(f'<a href="#{key}">{title}</a>' for key, title, _ in DOCS + TABLES)
    body = "".join(render_doc(path, key) for key, _, path in DOCS)
    body += "".join(render_csv(*entry) for entry in TABLES)
    script = """document.querySelectorAll('input[data-table]').forEach(input=>input.addEventListener('input',()=>{const q=input.value.trim().toLocaleLowerCase();document.querySelectorAll('#'+input.dataset.table+' tbody tr').forEach(row=>{row.hidden=!row.textContent.toLocaleLowerCase().includes(q)})}));"""
    html = (f'<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
            f'<title>客户端重建与回归规划 · 2026-10-18</title><style>{STYLE}</style><body><header>'
            '<div class="eyebrow">LUNA TRANSLATOR · CLIENT REBUILD PLAN · 2026-10-18</div>'
            '<h1>客户端重建与产品回归规划</h1><p>沿用原 PRD 与设计，接入精简 Core；本次补充内置日语分词与大模型学习设计。</p>'
            '<p class="scope">本轮仅规划。生产迁移、重建包与新产品回归均未执行；旧 Reader 与 Core 探针结果只作为基线。</p>'
            f'<div class="stats"><div><strong>56</strong>原产品需求</div><div><strong>22</strong>设计屏幕</div><div><strong>{validation["tasks"]}</strong>开发任务</div>'
            f'<div><strong>{validation["regression_cases"]}</strong>待执行回归项</div><div><strong>0</strong>本轮产品通过项</div></div>'
            '<p><a href="references/docs/design/first-product-v1/index.html">先查看：首版完整高保真交互稿</a></p>'
            f'<nav>{nav}</nav></header>'
            f'<main>{body}</main><footer>离线报告附原产品快照、任务及回归 CSV、Core ABI 参考和许可证。'
            '不包含真实游戏字幕、游戏截图、密钥或运行日志。原始产品确认状态未提升。'
            '<p id="baseline-notes">历史实机报告不随本规划打包；本报告只保留新旧接入边界与脱敏 Core 基线。原设计来源元数据已去除远端主机与个人绝对路径。</p>'
            f'<p><a href="gallery.html">打开原设计离线画廊</a> · <a href="report-manifest.json">查看文件哈希清单</a></p></footer><script>{script}</script></body></html>')
    (output / "index.html").write_text(html)
    records = [{"path": str(path.relative_to(output)), "bytes": path.stat().st_size,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
               for path in sorted(output.rglob("*")) if path.is_file()]
    manifest = {"report": "client-rebuild-plan", "date": "2026-10-18", "scope": "planning_only",
                "core_commit": CORE_COMMIT, "planning_revision": "1.1-language-study", "sections": len(DOCS) + len(TABLES), "validation": validation, "files": records}
    (output / "report-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
        for path in sorted(output.rglob("*")):
            if path.is_file():
                bundle.write(path, output.name + "/" + str(path.relative_to(output)))
    return {"html": str(output / "index.html"), "zip": str(archive), "zip_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "validation": validation}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "reports/client-rebuild-plan-20261018-language-study")
    args = parser.parse_args()
    try:
        print(json.dumps(build(args.output.resolve()), ensure_ascii=False, indent=2))
    except (OSError, ValueError, KeyError) as error:
        raise SystemExit(str(error))
