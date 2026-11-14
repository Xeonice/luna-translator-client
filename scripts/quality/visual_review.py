#!/usr/bin/env python3
"""Generate the one design/implementation report; never approve it automatically."""
from __future__ import annotations

import argparse
from html import escape
import json
import os
from pathlib import Path
import sys

from gate_core import safe_path
from gate_policy import atomic_write_json, load_context, read_json, validate_visual

ROOT = Path(__file__).resolve().parents[2]


def generate(root, manifest=None, *, output='reports/visual-comparison/index.html'):
    root = Path(root).resolve()
    target = safe_path(root, output)
    target.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(manifest, str):
        manifest = read_json(safe_path(root, manifest))
    if manifest is not None and (not isinstance(manifest, dict)
                                 or manifest.get('kind') != 'unified-visual-report/v1'):
        raise ValueError('visual.manifest_invalid')
    rows = []
    for row in (manifest or {}).get('rows', []):
        images = []
        for key, title in [('design_image', '已确认设计'), ('implementation_image', '真实实现')]:
            ref = row[key]
            image = safe_path(root, ref['path'])
            href = os.path.relpath(image, target.parent)
            images.append(f'<figure><figcaption>{title}</figcaption><a href="{escape(href, quote=True)}"><img src="{escape(href, quote=True)}" alt="{escape(str(row["scope"]))} · {title}"></a></figure>')
        rows.append('<section><h2>' + escape(str(row['scope'])) + '</h2><p>' +
                    escape(str(row.get('viewport'))) + ' · ' + escape(str(row.get('theme'))) +
                    ' · 状态 ' + escape(str(row.get('implementation_state'))) +
                    '</p><div class="pair">' + ''.join(images) + '</div><p>差异与确认：以当前报告的明确人工结论为准；生成页面不构成批准。</p></section>')
    body = ''.join(rows) or '<section><h2>尚无可审核的实现对照</h2><p>当前未提供实际设计 / 实现配对图。请先采集受影响页面与状态；不使用示例图、旧图或空模板声明视觉通过。</p></section>'
    html = '''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna · 设计与实现统一对照</title><style>
*{box-sizing:border-box}body{background:#f3f6fa;color:#1d2b40;font:14px/1.8 -apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;margin:0}main{max-width:1500px;padding:30px;margin:auto}h1{font-size:28px}section{background:white;border:1px solid #dce4ef;border-radius:12px;padding:22px;margin:22px 0}.pair{display:grid;grid-template-columns:1fr 1fr;gap:20px}figure{margin:0}figcaption{margin-bottom:10px;font-weight:600}img{display:block;width:100%;height:auto;border:1px solid #dce4ef}p{color:#617085}.pending{color:#925509}a{color:#235acf}@media(max-width:800px){.pair{grid-template-columns:1fr}main{padding:18px}}</style><main><p>LUNA / G09 VISUAL REVIEW</p><h1>设计与实现统一对照</h1><p class="pending">待人工确认 · 本页生成不会产生 approved 或 passed</p><p>同尺寸、同数据、同状态。你或指定审阅人的明确确认记录，必须绑定当前报告与实现；相关变化后旧确认失效。</p>'''
    target.write_text(html + body + '<p><a href="../gates/index.html">返回统一门禁结果</a></p></main></html>')
    return target


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=ROOT)
    p.add_argument('--manifest')
    p.add_argument('--review', help='Existing explicit human review; never generated here')
    p.add_argument('--change')
    p.add_argument('--register', action='store_true', help='Register existing evidence for the unified G09 gate')
    p.add_argument('--output', default='reports/visual-comparison/index.html')
    args = p.parse_args(argv)
    if args.register:
        if not args.manifest:
            p.error('--register requires a real --manifest')
        context = load_context(args.root, args.change)
        from development_gates import PREFIX, current_record, required_visual_scope
        from gate_checks import verified_scope as check_scope
        actual_paths, errors = check_scope(args.root, context)
        if errors:
            raise ValueError('; '.join(errors))
        required = required_visual_scope(args.root, actual_paths)
        if not required:
            raise ValueError('visual.no_registered_UI_impact: no visual approval will be manufactured')
        verdict = validate_visual(args.root, args.manifest, args.review, required_scope=required,
                                  authorized_reviewers={'user'})
        record = current_record(args.root, context['change_id'])
        record['visual'] = {'manifest': args.manifest, 'review': args.review}
        atomic_write_json(args.root, f"{PREFIX}/{context['change_id']}/latest.json", record)
        print(json.dumps(verdict, ensure_ascii=False))
        return 0 if verdict['status'] == 'passed' else 1
    target = generate(args.root, args.manifest, output=args.output)
    print(json.dumps({'report': str(target), 'review_decision': 'pending',
                      'visual_approval_created': False, 'product_acceptance_promoted': False}, ensure_ascii=False))
    return 0  # Report generation only, never a visual qualification verdict.


if __name__ == '__main__':
    raise SystemExit(main())
