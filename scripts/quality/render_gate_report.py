#!/usr/bin/env python3
"""One readable gate report; never changes a verdict or creates an approval."""
from html import escape
import json
import os
from pathlib import Path

NAMES = ('需求与产品范围', '模块边界', '目录与修改范围', '技术栈与资源', '真实编译',
         '功能与交互', '数据库与存储', '正式包与实机', '视觉对照与人工确认')


def render(root, result, record=None):
    root = Path(root)
    target = root / 'reports/gates/index.html'
    target.parent.mkdir(parents=True, exist_ok=True)
    record = record or {}
    rows = []
    for i, gate in enumerate(result.get('gates', [])):
        status = gate.get('status', 'not_run')
        label = status if gate.get('required', True) else '不适用 · 本次改动未涉及'
        detail = escape(json.dumps(gate.get('details'), ensure_ascii=False, indent=2))
        rows.append(f'<tr><td>{escape(gate.get("gate_id", ""))} · {NAMES[i]}</td>'
                    f'<td><span class="{escape(status)}">{escape(label)}</span></td>'
                    f'<td>{escape(gate.get("reason", ""))}<details><summary>检查详情</summary><pre>{detail}</pre></details></td></tr>')
    logs = []
    for receipts in record.get('receipts', {}).values():
        for receipt in receipts:
            links = []
            for stream in ('stdout', 'stderr'):
                ref = receipt.get(stream, {})
                if ref.get('path'):
                    links.append(f'<a href="../../{escape(ref["path"], quote=True)}">{stream}</a>')
            logs.append(f'<li>{escape(receipt.get("check_id", ""))} · exit {receipt.get("exit_code")} · '
                        f'实际测试 {receipt.get("test_count", 0)} · {" / ".join(links)}</li>')
    qualification = ''
    external = None
    for receipts in record.get('receipts', {}).values():
        for receipt in receipts:
            argv = receipt.get('command', [])
            if receipt.get('check_id') == 'gate-effectiveness-execution' and '--output' in argv:
                external = root / argv[argv.index('--output') + 1] / 'verification.json'
    if external is not None and external.is_file():
        try:
            payload = json.loads(external.read_text())
            samples = sum(len(row.get('observations', [])) for row in payload.get('scenarios', []))
            checks = sum(row.get('assertion_count', 0) for row in payload.get('scenarios', []))
            groups = []
            for row in payload.get('scenarios', []):
                groups.append('<tr><td>' + escape(row['id']) + '</td><td>' + escape(row.get('title', '')) +
                              '</td><td class="' + escape(row['status']) + '">' + escape(row['status']) +
                              '</td><td>' + str(row.get('assertion_count', 0)) + '</td></tr>')
            href = os.path.relpath(external, target.parent)
            qualification = ('<div class="box"><h2>32组门禁有效性</h2><p>实际结果：' +
                escape(str(payload.get('counts'))) + '；断言 ' + str(checks) +
                '；观察记录 ' + str(samples) + '；运行期间源稳定：' +
                escape(str(payload.get('source_unchanged'))) + '</p><p>这些是隔离门禁实验及实际生产端口/Hook证据核验，不是正式产品验收。</p>' +
                '<details><summary>逐组结果</summary><div class="table"><table><thead><tr><th>场景</th><th>验证内容</th><th>结果</th><th>断言</th></tr></thead><tbody>' +
                ''.join(groups) + '</tbody></table></div></details><a href="' + escape(href, quote=True) +
                '">查看机器原件与全部命令</a></div>')
        except (ValueError, OSError):
            qualification = '<p class="failed">有效性结果损坏，不能记为通过。</p>'
    html = '''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna · 统一门禁结果</title><style>
*{box-sizing:border-box}body{margin:0;background:#f3f6fa;color:#1d2b40;font:14px/1.8 -apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif}main{max-width:1200px;margin:auto;padding:38px 25px}h1{font-size:30px}p{color:#5b6b80}table{border-collapse:collapse;width:100%;background:white}th,td{padding:15px;text-align:left;border-bottom:1px solid #dce4ef;vertical-align:top}th{background:#e9eff8}pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px}details{margin:10px 0}.box{background:white;border:1px solid #dce4ef;border-radius:12px;padding:20px;margin:20px 0}.passed{color:#17644c}.failed{color:#a33343}.blocked{color:#925509}.not_run{color:#617085}a{color:#235acf}.table{overflow:auto}summary{cursor:pointer}code{background:#edf1f7;padding:3px 5px}
</style><main><p>LUNA / QUALITY GATES</p><h1>统一门禁结果</h1>'''
    html += f'<p>工作项：{escape(str(result.get("change_id")))} · 运行：{escape(str(result.get("run_id")))} · 阶段：{escape(str(result.get("stage")))}</p>'
    html += '<div class="box"><strong>本次工作项检查：' + escape(result.get('status', 'not_run')) + '</strong><p>工程或门禁自测通过不代表产品通过。当前正式产品验收、人工视觉批准与真实四游戏结果独立判断；待审、失败和未执行均不能准出。</p></div>'
    html += '<div class="table"><table><thead><tr><th>门禁</th><th>状态</th><th>原因与下一步</th></tr></thead><tbody>' + ''.join(rows) + '</tbody></table></div>'
    html += '<div class="box"><h2>视觉对照</h2><p>只维护<a href="../visual-comparison/index.html">一份设计 / 实现统一报告</a>。涉及 UI 时，缺当前报告或明确确认就不能通过 G09。本次工作项不适用，与门禁自身正反例是否执行、产品是否验收分别判断。</p></div>'
    html += qualification + '<details><summary>本轮实际命令与原始日志</summary><ul>' + ''.join(logs) + '</ul></details>'
    html += '<div class="box"><h2>接入边界</h2><p>Codex 正常持久信任、真实 L3、Git 本地安装和远端必需检查分别取证；现有长会话不能据新会话推断全部工具路径热加载。Git 元数据、YAML 或同名绿色任务不能替代当前模板与受保护分支的实际执行。</p><p>本页工作项结果不自动批准产品。正式包、四游戏、真实翻译与人工视觉仍按各自原件判定。<a href="../../docs/development/github-activation.md">查看 GitHub 接入与 G06～G09 验证方法</a></p></div>'
    html += '<p><a href="../../docs/development/gate-validation-design.html">门禁设计与32组场景</a> · <a href="../../docs/development/engineering-gates.md">当前执行规则</a></p></main></html>'
    target.write_text(html)
    return target
