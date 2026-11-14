#!/usr/bin/env python3
"""Render actual batch records; never manufacture a test or game verdict."""
from __future__ import annotations
import argparse
import hashlib
import html
import json
import os
import shutil
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[2]
LABELS = {'passed':'通过','failed':'失败','blocked':'受阻','not_run':'未执行','in_progress':'进行中','implemented':'已实现','partial':'部分实现','not_assessed':'待核对'}

def escape(value):
    return html.escape(str(value), quote=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    record = args.record.resolve()
    output = args.output.resolve()
    if not record.is_relative_to(ROOT/'.build') or not output.is_relative_to(ROOT/'reports'):
        parser.error('record must be under .build; report must be under reports')
    if output.exists():
        parser.error('report output must be fresh; previous evidence is preserved')
    facts = json.loads(record.read_text())
    if facts.get('kind') != 'reading_session_batch_execution' or facts.get('schema_version') != 1:
        parser.error('unknown actual batch record format')
    output.mkdir(parents=True)
    def badge(status):
        return f'<span class="badge {escape(status)}">{escape(LABELS.get(status,status))}</span>'
    def links(refs, bundle=True):
        result=[]
        for ref in refs:
            if Path(ref).is_absolute() or '..' in Path(ref).parts: raise ValueError('Evidence paths must be repository relative')
            path=(ROOT/ref).resolve()
            if not path.is_relative_to(ROOT) or not path.is_file() or path.is_symlink():
                raise ValueError('missing or unsafe actual evidence: '+ref)
            if bundle:
                if path.stat().st_size > 64 * 1024 * 1024: raise ValueError('Select a bounded receipt/image instead of embedding a full package: '+ref)
                local = output/'evidence'/ref
                local.parent.mkdir(parents=True,exist_ok=True)
                shutil.copy2(path,local)
            else:
                local = path
            result.append(f'<a href="{escape(os.path.relpath(local,output))}">{escape(ref)}</a>')
        return '<br>'.join(result)
    def table(rows,columns):
        head=''.join(f'<th>{escape(label)}</th>' for _,label in columns)
        body=[]
        for row in rows:
            cells=[]
            for key,_ in columns:
                value=row.get(key,'')
                cell=badge(value) if key in {'status','implementation','alignment','runtime'} else links(value) if key=='evidence' else escape(value)
                cells.append('<td>'+cell+'</td>')
            body.append('<tr>'+''.join(cells)+'</tr>')
        return '<div class="table-wrap"><table><thead><tr>'+head+'</tr></thead><tbody>'+''.join(body)+'</tbody></table></div>'
    sections=[]
    sections.append('<section><h2>本批范围与结果边界</h2><p>'+escape(facts['scope'])+'</p><p>'+escape(facts.get('limitations',''))+'</p></section>')
    sections.append('<section><h2>功能实现与对齐</h2>'+table(facts.get('features',[]),[('name','功能'),('implementation','实现'),('alignment','设计核对'),('runtime','最终包验证'),('detail','结果与差距'),('evidence','证据')])+'</section>')
    sections.append('<section><h2>逐层验证</h2>'+table(facts.get('checks',[]),[('name','检查'),('status','实际结果'),('detail','执行与限制'),('evidence','回执')])+'</section>')
    sections.append('<section><h2>四款游戏回归</h2><p>SR、Memories Off、Never7、サクラノ詩 10th；Ever17 不在本次范围。历史包结果不能代替本批候选包。</p>'+table(facts.get('games',[]),[('name','游戏'),('status','实际结果'),('detail','原文、译文与交互'),('evidence','现场证据')])+'</section>')
    screenshots = []
    for shot in facts.get('screenshots', []):
        ref = shot['path']
        if Path(ref).suffix.lower() not in {'.png', '.jpg', '.jpeg'}:
            raise ValueError('Runtime screenshot must be a PNG or JPEG')
        evidence_link = links([ref])
        local_ref = Path('evidence') / ref
        screenshots.append('<figure><a href="'+escape(local_ref.as_posix())+'"><img loading="lazy" style="display:block;max-width:100%;height:auto;border-radius:10px" src="'+escape(local_ref.as_posix())+'"></a><figcaption>'+escape(shot['caption'])+'<br>'+evidence_link+'</figcaption></figure>')
    if screenshots:
        sections.append('<section><h2>实际窗口与对白</h2>'+''.join(screenshots)+'</section>')
    sections.append('<section><h2>独立审查与修复</h2>'+table(facts.get('review_findings',[]),[('id','编号'),('status','收尾'),('detail','问题及修正'),('evidence','证据')])+'</section>')
    identity=escape(json.dumps(facts.get('candidate',{}),ensure_ascii=False,indent=2))
    sections.append('<section><h2>制品、源码与保留内容</h2><pre>'+identity+'</pre><p>'+escape(facts.get('preservation',''))+'</p>'+links(facts.get('deliverables',[]),False)+'</section>')
    source_sha=hashlib.sha256(record.read_bytes()).hexdigest()
    document='''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna · 阅读与会话修订执行报告</title><style>
    :root{color-scheme:dark;font:14px/1.65 -apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#e4e5eb;background:#121722}*{box-sizing:border-box}body{margin:0;background:radial-gradient(ellipse at 70% 0,#303a5044,transparent 60%);padding:48px 5vw 70px}main{max-width:1360px;margin:auto}header{margin-bottom:32px}.eyebrow{font-size:12px;color:#9ba6bc;letter-spacing:.12em}h1{font-size:32px;letter-spacing:-.04em;font-weight:600;margin:9px 0}h2{font-size:19px;font-weight:600;margin:0 0 16px}p{color:#b4bbca}section{background:#252c3b88;border:1px solid #ffffff14;border-radius:16px;padding:26px;margin:18px 0;backdrop-filter:blur(24px)}a{color:#bfd0ef;text-decoration:none;overflow-wrap:anywhere}a:hover{text-decoration:underline}.table-wrap{overflow:auto}table{border-collapse:collapse;width:100%;min-width:750px}th{font-size:12px;color:#9da8bd;text-align:left;font-weight:500}td,th{vertical-align:top;padding:14px 12px;border-bottom:1px solid #ffffff0d}td{font-size:13px}td:first-child{min-width:130px}td:last-child{max-width:300px;font-size:11px}.badge{display:inline-block;padding:2px 8px;border-radius:6px;white-space:nowrap;background:#ffffff0c;color:#b6bfce;font-size:12px}.passed,.implemented{color:#c1d6c7;background:#839f8620}.failed{color:#e4b9b7;background:#af68631c}.blocked,.partial{color:#d7c7a9;background:#ae97601a}pre{font:11px/1.7 ui-monospace,SFMono-Regular,monospace;white-space:pre-wrap;overflow-wrap:anywhere;color:#bac3d3}footer{font-size:11px;color:#7d879b;margin-top:28px}@media(max-width:760px){body{padding:25px 15px}section{padding:18px}h1{font-size:25px}}
    </style><main><header><div class="eyebrow">LUNA · 实际执行记录</div><h1>阅读与会话修订</h1><p>'''+escape(facts.get('updated_at',''))+''' · '''+badge(facts.get('status','in_progress'))+'''</p></header>'''+''.join(sections)+f'<footer>原始运行记录 SHA256：{source_sha}<br>报告只展示已记录证据；未执行、受阻与失败分别保留。未提交或推送代码。</footer></main></html>'
    (output/'index.html').write_text(document)
    (output/'execution.json').write_text(json.dumps(facts,ensure_ascii=False,indent=2)+'\n')
    with zipfile.ZipFile(output.with_suffix('.zip'),'w',compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(output.rglob('*')):
            if path.is_file(): archive.write(path,output.name+'/'+path.relative_to(output).as_posix())
    print(json.dumps({'report':str((output/'index.html').relative_to(ROOT)),'archive':str(output.with_suffix('.zip').relative_to(ROOT)),'record_sha256':source_sha},ensure_ascii=False))

if __name__=='__main__':
    main()
