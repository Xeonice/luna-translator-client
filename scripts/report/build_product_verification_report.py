#!/usr/bin/env python3
"""Package actual execution receipts, separate from planned acceptance cases."""
import hashlib
import html
import json
from pathlib import Path
import re
import shutil
from datetime import datetime, timezone
import argparse

ROOT=Path(__file__).resolve().parents[2]
OUT=None
def escape(value):return html.escape(str(value))
def load(path):return json.loads(path.read_text())
def source_identity(data):
    artifact=data.get('artifact',{})
    return data.get('source_tree_sha256') or (artifact.get('source_tree_sha256') if isinstance(artifact,dict) else None)
def source_relation(identity,packaged):
    return 'unbound_not_package_acceptance' if not identity else ('same_packaged_source' if identity==packaged else 'historical_different_source')
def evidence(path,label):
    if not path.is_file():return {'id':label,'status':'not_run'}
    target=OUT/'evidence'/f'{label}{path.suffix}'
    target.parent.mkdir(exist_ok=True,parents=True);shutil.copyfile(path,target)
    return {'id':label,'file':str(target.relative_to(OUT)),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
def main():
    global OUT
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path)
    parser.add_argument('--gate',type=Path)
    parser.add_argument('--ui',type=Path)
    parser.add_argument('--native',type=Path,action='append',default=[])
    parser.add_argument('--evidence',action='append',default=[],metavar='LABEL=PATH')
    args=parser.parse_args()
    OUT=(args.output or ROOT/'reports'/('first-product-implementation-'+datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ'))).resolve()
    if OUT.exists():parser.error('Output exists; prior evidence will not be overwritten')
    OUT.mkdir(parents=True,exist_ok=True)
    manifest=load(ROOT/'dist/build-manifest.json') if (ROOT/'dist/build-manifest.json').is_file() else {}
    packaged=manifest.get('source_tree_sha256')
    rows=[]
    # Explicit inputs replace dated defaults; component logs do not establish package identity.
    logs={}
    if args.evidence:
        logs={}
        for pair in args.evidence:
            label,separator,name=pair.partition('=')
            if not separator or not re.fullmatch(r'[a-z0-9][a-z0-9-]*',label) or label in logs:parser.error('Use a unique safe LABEL=PATH')
            logs[label]=name
    for label,name in logs.items():
        row=evidence(ROOT/name,label)
        if row.get('file'):
            text=(ROOT/name).read_text(errors='replace')
            results=re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text)
            row['passed_tests']=sum(int(r[1]) for r in results)
            row['failed_tests']=sum(int(r[2]) for r in results)
            row['ignored_tests']=sum(int(r[3]) for r in results)
            row['status']='failed' if row['failed_tests'] or re.search(r'^error[:\[]|^error:|panicked at',text,re.M) else 'recorded_component_result_not_package_acceptance'
        row['source_relation']='unbound_not_package_acceptance'
        rows.append(row)
    gates=[]
    if args.gate:gates=[args.gate]
    else:
        for path in (ROOT/'.build/quality').glob('*.json'):
            try:gate=load(path)
            except (OSError,ValueError):continue
            if isinstance(gate.get('steps'),list) and gate.get('profile') and source_identity(gate)==packaged:gates.append(path)
        gates.sort(key=lambda p:p.stat().st_mtime_ns)
    if gates:
        gate=load(gates[-1])
        if not isinstance(gate.get('steps'),list) or not gate.get('profile'):parser.error('--gate must be a full engineering gate receipt')
        row=evidence(gates[-1],'engineering-gate');row.update(status=gate.get('status','not_run'),source_tree_sha256=source_identity(gate),source_relation=source_relation(source_identity(gate),packaged));rows.append(row)
    else:rows.append({'id':'engineering-gate','status':'not_run','source_relation':'no_gate_for_packaged_source'})
    artifact_paths=[(ROOT/'dist/build-manifest.json','build-identity')]+[(path,f'native-run-{index}') for index,path in enumerate(args.native or [ROOT/'.build/packaged-product-regression/run.json',ROOT/'.build/hud-interaction-result.json'],1)]
    for path,label in artifact_paths:
        row=evidence(path,label)
        if path.is_file():
            value=load(path);identity=source_identity(value);row.update(status=value.get('status','recorded_not_acceptance'),source_tree_sha256=identity,source_relation=source_relation(identity,packaged))
            if label!='build-identity' and identity!=packaged:row['acceptance_status']='not_current_package_evidence'
        rows.append(row)
    if args.ui:
        ui=load(args.ui);row=evidence(args.ui,'controlled-ui')
        source_map=ui.get('sourceSha256',{})
        drift=[name for name,expected in source_map.items() if not (ROOT/name).is_file() or hashlib.sha256((ROOT/name).read_bytes()).hexdigest()!=expected]
        row.update(status=ui.get('status','not_run'),behavior_status=ui.get('behaviorStatus'),visual_status=ui.get('visualStatus'),source_relation='same_current_ui_sources_only' if source_map and not drift and not ui.get('sourceDrift') else 'stale_or_unbound_ui_sources',stale_sources=drift,behavior_passed=sum(item.get('status')=='passed' for item in ui.get('behavior',[])),behavior_total=len(ui.get('behavior',[])),visual_total=len(ui.get('captures',[])))
        gallery=OUT/'evidence/ui';gallery.mkdir()
        for capture in ui.get('captures',[]):
            for key in ['candidate','baselineCopy','diff']:
                name=capture.get(key)
                if name and Path(name).name==name and (args.ui.parent/name).is_file():shutil.copyfile(args.ui.parent/name,gallery/name)
        if (args.ui.parent/'index.html').is_file():shutil.copyfile(args.ui.parent/'index.html',gallery/'index.html');row['gallery']='evidence/ui/index.html'
        rows.append(row)
    data={'schema_version':2,'executed_at':datetime.now(timezone.utc).isoformat(),'packaged_source_tree_sha256':packaged,'overall_status':'in_progress_not_release_accepted','plan_cases_unchanged':True,'results':rows,'limitations':['完整STORAGE-01..22 adapter及431字段真实Rust映射尚未验收；局部字段和边界测试不能代替完整存储验收。','真实产品视觉差异保持失败；不修改确认图或阈值。','same_packaged_source表示整树身份一致；same_current_ui_sources_only仅表示受控UI输入一致。历史不同源码的记录不计入当前包验收。','正式包回归、资源/协议测试、受控UI行为和门禁自测分别理解。','未执行的复制/删除闭包、安全导入导出、显式回退/预算、游戏启动与偏好继承、全局快捷键不得按入口算通过。','未提交或推送，当前是开发候选，不能作为首版上线验收通过。']}
    (OUT/'verification.json').write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    body=''.join(f'<tr><td>{escape(r["id"])}</td><td>{escape(r.get("status","recorded_not_acceptance"))}</td><td>{escape(r.get("source_relation","unbound_not_package_acceptance"))}</td><td>'+ (f'<a href="{escape(r["file"])}">回执／日志</a>' if r.get('file') else '未执行')+(f' · <a href="{escape(r["gallery"])}">视觉差异</a>' if r.get('gallery') else '')+'</td></tr>' for r in rows)
    notes=''.join(f'<li>{escape(n)}</li>' for n in data['limitations'])
    (OUT/'index.html').write_text(f'''<!doctype html><html lang="zh-CN"><meta charset="UTF-8"><title>Luna 首版开发与实际验证</title><style>body{{font:15px -apple-system,BlinkMacSystemFont,sans-serif;background:#171b24;color:#e6e7ea;max-width:1080px;margin:60px auto;padding:0 28px;line-height:1.65}}h1{{font-size:30px}}p,li{{color:#bbc0ca}}table{{width:100%;border-collapse:collapse;background:#252a35;border-radius:12px}}td,th{{padding:12px 18px;text-align:left;border-bottom:1px solid #414754}}a{{color:#dce5f6}}.status{{padding:16px 20px;background:#34313a;border-radius:12px}}small{{color:#a8adb8}}</style><h1>Luna 首版开发与实际验证</h1><p class="status">当前状态：开发与集成回归进行中，尚未通过首版上线验收。</p><p>本报告归档本轮真实执行结果。计划用例保持原定义；没有运行的用例不填通过。日志中的独立测试有交集，表内数量不能直接相加当作产品用例覆盖。</p><table><thead><tr><th>验证范围</th><th>实际状态</th><th>源码归属</th><th>证据</th></tr></thead><tbody>{body}</tbody></table><h2>交付边界与剩余项</h2><ul>{notes}</ul><p><a href="verification.json">机器可读索引与SHA-256</a> · <a href="../../docs/development/iterations/FP1-IMPLEMENT-20261019/plan.md">本轮迭代卡</a></p><small>{escape(data['executed_at'])} · 新Core ABI1 / JSONL v2 · 原版权与许可保留</small></html>''')
    archive=shutil.make_archive(str(OUT), 'zip', OUT)
    print(json.dumps({'html':str(OUT/'index.html'),'archive':archive,'status':data['overall_status']},ensure_ascii=False))
if __name__=='__main__':main()
