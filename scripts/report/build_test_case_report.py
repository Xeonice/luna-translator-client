#!/usr/bin/env python3
"""Build a self-contained, filterable planning report; never runs the product."""
import csv
import hashlib
import html
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/quality'))
from build_engineering_baseline_report import storage_enforcement_section
from test_plan_contract import validate_plan
from test_operations_contract import validate_operations, dependencies_for

OUTPUT = ROOT / 'reports/first-product-test-cases-20261019.html'


def esc(value):
    return html.escape(str(value), quote=True)


def items(values):
    return '<ul>' + ''.join('<li>'+esc(value)+'</li>' for value in values) + '</ul>'


def inline_json(value):
    # Keep external/model test strings inert even if future cases contain HTML.
    return json.dumps(value, ensure_ascii=False).replace('<', '\\u003c').replace('>', '\\u003e').replace('&', '\\u0026')


def main():
    storage_html = storage_enforcement_section(ROOT)
    errors = validate_plan(ROOT) + validate_operations(ROOT)
    if errors:
        raise SystemExit('\n'.join(errors))
    if OUTPUT.exists():
        raise SystemExit('Report already exists; preserve the previous report before generating a new revision.')
    plan_path = ROOT / 'docs/development/first-product-test-plan.json'
    plan = json.loads(plan_path.read_text())
    features = json.loads((ROOT / 'scripts/report/test_case_feature_summaries.json').read_text())
    if isinstance(features, dict):
        features = features['features']
    if {f['id'] for f in features} != {f'FP-{i:02}' for i in range(1,13)}:
        raise SystemExit('Feature overview must cover exactly FP-01 through FP-12.')
    variants = {v['id']: v for v in plan['variants']}
    environments = {v['id']: v for v in plan['environments']}
    layers = {v['id']: v for v in plan['layers']}
    count = len(plan['cases'])
    executions = sum(len(c['variant_ids']) for c in plan['cases'])
    ops = json.loads((ROOT/'docs/development/first-product-test-operations.json').read_text())
    verification_path = ROOT/'.build/quality/test-operations-verification.json'
    verification = json.loads(verification_path.read_text()) if verification_path.is_file() else {}
    operations_check_text = ('前次'+str(verification.get('operations_guard_tests'))+'项前置/维护守卫与TypeScript/Rust编译通过，仅证明规划与工程设施') if verification.get('status')=='passed' else '尚无本轮工程检查回执；报告不声明产品通过'
    areas = {row['id']:row for row in ops['maintenance_areas']}
    case_areas = {row['case_id']:row['area_ids'] for row in ops['case_maintenance']}
    affected = {row['id']:[] for row in ops['prerequisites']}
    dependency_variants = {}
    for case in plan['cases']:
        by_prerequisite = {}
        for variant in case['variant_ids']:
            for row in dependencies_for(case,variant,ops):
                affected[row['id']].append(case['id']+'@'+variant)
                by_prerequisite.setdefault(row['id'],[]).append(variant)
        dependency_variants[case['id']] = by_prerequisite
    providers = {'user':'用户授权/提供','team':'研发侧准备','shared':'双方配合'}
    readiness = {'pending_review':'待评审','needs_confirmation':'待确认','prior_evidence_recheck':'历史证据待复验','team_preparation_pending':'团队待交付','not_configured':'未配置'}
    maintenance_states = {'existing':'已有工程设施','mixed':'部分已有，产品部分计划','planned':'计划 / 未实现','generated':'运行时生成，归档待配置','external':'独立仓库职责，本轮未验证'}
    prerequisite_rows = []
    for row in ops['prerequisites']:
        prerequisite_rows.append(f'<tr data-provider="{esc(row["provider"])}"><td><code>{esc(row["id"])}</code><strong>{esc(row["title"])}</strong></td><td>{esc(providers[row["provider"]])}<br><span class="muted">{esc(" / ".join(row["owners"]))}</span></td><td><p>{esc(row["deliverable"])}</p><details><summary>你要配合什么 / 如何核验</summary><h4>用户配合</h4><p>{esc(row["user_action"])}</p><h4>解除依赖的证据</h4><p>{esc(row["validation"])}</p><p class="note">{esc(row["current_evidence"])}</p></details></td><td>{esc(" / ".join(row["required_by_gates"]))}<br><span class="status">{esc(readiness[row["status"]])}</span></td><td>{len(affected[row["id"]])} 个组合<br><button class="small" data-select-prereq="{esc(row["id"])}">查看受影响用例</button></td></tr>')
    maintenance_rows = ''.join('<tr><td><code>'+esc(row['id'])+'</code><br>'+esc(row['title'])+'</td><td><code>'+esc(row['path'])+'</code></td><td>'+esc(maintenance_states[row['state']])+'</td><td>'+esc(' / '.join(row['owners']))+'</td><td>'+esc(row['responsibility'])+'<br><span class="muted">'+esc(row['trigger'])+'</span></td><td>'+esc(row['command_status'])+'</td></tr>' for row in ops['maintenance_areas'])
    prerequisite_options = ''.join(f'<option value="{esc(row["id"])}">{esc(row["id"]+" · "+row["title"])}</option>' for row in ops['prerequisites'])
    operations_html = f'''<section class="chapter" id="prerequisites"><h2>开始测试前，要准备什么</h2><p class="section-lead">18项准备按组合生效；当前状态都是待确认、准备或评审，不代表实际就绪。规则/受控协议/离线开发不需要等全部账号与游戏到齐。双屏只影响3个DISPLAYS组合，多Key身份只影响MC-04专项。</p><div class="prep-toolbar"><label>准备责任 <select id="prereq-provider"><option value="">全部准备项</option><option value="user">需要用户授权或配合</option><option value="team">研发侧自行准备</option></select></label><span id="prep-count" aria-live="polite">18 / 18 项</span></div><div class="table-wrap"><table id="prerequisite-table"><thead><tr><th>准备项</th><th>谁提供 / 主责</th><th>具体交付物及解除方法</th><th>最晚门禁 / 当前状态</th><th>影响范围</th></tr></thead><tbody>{''.join(prerequisite_rows)}</tbody></table></div><p class="note">不要通过聊天、HTML、CSV或源码提供API Key、密码、证书私钥或公证密码。通过客户端设置、系统安全库或受控CI secret配置，报告只登记非秘密元数据和脱敏引用。现有Mac/游戏/模型/签名授权本轮均未重新核验；缺前置时在实际执行attempt记blocked，产品计划仍not_run。</p></section><section class="chapter" id="maintenance"><h2>测试代码、基准和证据维护在哪里</h2><p class="section-lead">模块Owner维护所属测试逻辑，QA维护计划/覆盖/证据和发布汇总。计划目录不是已实现runner，不预建空目录冒充完成。组件/视觉沿既有apps/desktop/tests目标，正式包、原生、游戏和质量脚本细分在既有tests/integration下。</p><div class="maintenance-tree"><pre>docs/development/   计划、前置和用例映射（已有）
openspec/           需求/场景 → 测试与执行器（已有机制）
scripts/quality/    工程守卫；不是产品回归（已有）
scripts/report/     报告生成与报告Playwright QA源码（已有）
crates/*/src/       就近 #[cfg(test)] 领域单测
crates/*/tests/     crate合法公开边界；app-infra/tests验真实I/O
apps/desktop/tests/ components / visual / support（产品harness计划）
tests/contracts/    共享跨进程契约（计划）
tests/integration/  application / desktop / native / games / quality（计划）
tests/fixtures/     合成或授权样本、版本/来源许可（计划）
.build/             运行输出与缓存，不能作为长期测试源码或唯一证据
受控制品存储/        run_id原始证据备份，访问/保留政策待PRE-15冻结
reports/            脱敏生成报告</pre></div><div class="table-wrap"><table id="maintenance-table"><thead><tr><th>维护对象</th><th>准确位置</th><th>当前状态</th><th>主责</th><th>职责与触发</th><th>命令 / 设施状态</th></tr></thead><tbody>{maintenance_rows}</tbody></table></div><p class="note">每次变更跑规范/工程门禁和受影响实际测试；ABI/DTO/schema/资源变更增加消费兼容/迁移回归；阶段交付验证对应切片；每个最终RC收齐同包全部必需组合。失败由模块Owner修复并保留首次失败及新attempt。批准图/期望只有依据变更和明确评审后更新，不能为迁就实现自动重拍。当前无Git/remote，CI仅配置文件，不在本轮提交推送。</p><p>运行原始证据从临时目录复制到受控存储，稳定run_id、hash和访问策略一起保存；保留期限/预算待B0确认，不默认上传私人游戏文本。reports只放审核后的脱敏副本。独立Core内部测试由HK在Core仓库维护，客户端仍须验证消费兼容和实际游戏。</p></section>'''
    feature_cards = []
    for feature in features:
        matched = [c for c in plan['cases'] if feature['id'] in c['feature_ids']]
        feature_cards.append(f'''<details class="feature-card" id="{esc(feature['id'])}"><summary><span class="eyebrow">{esc(feature['id'])} · {len(matched)} 个关联定义</span><h3>{esc(feature['title'])}</h3><p>{esc(feature['user_journey'])}</p><span class="disclosure">展开验证路径</span></summary><div class="feature-body"><h4>计划怎么验证</h4>{items(feature['verification_focus'])}<h4>怎样判断通过</h4>{items(feature['observable_pass'])}<h4>需要保留的证据</h4>{items(feature['evidence'])}<div class="note">{items(feature['special_notes'])}</div><button class="secondary" data-select-feature="{esc(feature['id'])}">查看这项功能的 {len(matched)} 个用例</button></div></details>''')
    cases = []
    for case in plan['cases']:
        search = ' '.join([case['id'],case['title'],case['owner'],case['method'],*case['feature_ids'],*case['ai_config_ids'],*case['module_ids'],*case['component_ids'],*case['variant_ids'],*case['preconditions'],*case['steps'],*case['expected']])
        combos = []
        for variant in case['variant_ids']:
            v = variants[variant]
            label = '同源构建 / 规则或集成证据' if v['evidence_scope']=='source_build' else '同一最终 RC / 实际安装包'
            envs = items([environments[e]['description'] for e in v['environment_ids']])
            dependency_names = [row['id']+' · '+row['title'] for row in dependencies_for(case,variant,ops)]
            combos.append(f'<details class="variant" data-variant="{esc(variant)}"><summary><code>{esc(case["id"]+"@"+variant)}</code><span>{esc(label)}</span></summary>{envs}<h4>该组合需要的准备</h4>{items(dependency_names)}</details>')
        source = f'继承 {case["source"]["id"]} · {case["source"]["path"]}' if case['source'] else '新增机制验证；展开已有首版承诺'
        override = '<div class="note"><strong>本版验收替代</strong><p>'+esc(case['acceptance_override'])+'</p></div>' if case.get('acceptance_override') else ''
        variant_policy = '<div class="note">'+esc(case['variant_policy'])+'</div>' if case.get('variant_policy') else ''
        cases.append(f'''<details class="test-case" id="case-{esc(case['id'])}" data-features="{esc(' '.join(case['feature_ids']))}" data-layer="{esc(case['layer'])}" data-variants="{esc(' '.join(case['variant_ids']))}" data-dependency-variants="{esc(json.dumps(dependency_variants[case['id']],ensure_ascii=False))}" data-search="{esc(search.lower())}"><summary><span class="case-top"><code>{esc(case['id'])}</code><span class="badge">{esc(case['layer'])} · {esc(case['gate'])}</span><span class="status">未执行</span></span><strong>{esc(case['title'])}</strong><span class="case-sub">{esc(' / '.join(case['feature_ids']))} · 主责 {esc(case['owner'])} · {len(case['variant_ids'])} 个环境组合</span></summary><div class="case-body"><p class="source">{esc(source)}</p><div class="note"><strong>测试源码维护区域（未落地部分为计划）</strong>{items([area_id+' · '+areas[area_id]['path']+' · '+ ' / '.join(areas[area_id]['owners']) for area_id in case_areas[case['id']]])}</div><div class="contract"><span>{esc(layers[case['layer']]['title'])}</span><span>模块 {esc(' / '.join(case['module_ids']))}</span><span>组件 {esc(' / '.join(case['component_ids']))}</span></div><div class="columns"><section><h4>执行前置</h4>{items(case['preconditions'])}<h4>验证方式</h4><p>{esc(case['method'])}</p></section><section><h4>操作步骤</h4><ol>{''.join('<li>'+esc(step)+'</li>' for step in case['steps'])}</ol></section><section><h4>预期结果 / 通过判断</h4>{items(case['expected'])}</section></div>{override}{variant_policy}<div class="columns two"><section><h4>证据要求</h4>{items(case['evidence_types'])}</section><section><h4>逐环境执行身份</h4>{''.join(combos)}</section></div><p class="note">状态为计划中的 not_run；此处的步骤、环境和预期不是实际运行记录。缺前置记 blocked，执行不符合预期记 failed，证据齐全才可在独立回执中记 passed。</p></div></details>''')
    feature_options=''.join(f'<option value="{esc(f["id"])}">{esc(f["id"]+" · "+f["title"])}</option>' for f in features)
    layer_options=''.join(f'<option value="{esc(l["id"])}">{esc(l["id"]+" · "+l["title"])}</option>' for l in plan['layers'])
    variant_options=''.join(f'<option value="{esc(v)}">{esc(v)}</option>' for v in variants)
    downloads = {'plan': {'name':'first-product-test-plan.json','mime':'application/json','content':plan_path.read_text()}, 'executions':{'name':'first-product-test-executions.csv','mime':'text/csv;charset=utf-8','content':(ROOT/'docs/development/first-product-test-executions.csv').read_text()}, 'coverage':{'name':'first-product-test-coverage.csv','mime':'text/csv;charset=utf-8','content':(ROOT/'docs/development/first-product-test-coverage.csv').read_text()}}
    downloads['storage']={'name':'storage-acceptance.json','mime':'application/json','content':(ROOT/'docs/development/storage-acceptance.json').read_text()}
    downloads['operations']={'name':'first-product-test-operations.json','mime':'application/json','content':(ROOT/'docs/development/first-product-test-operations.json').read_text()}
    downloads['dependencies']={'name':'first-product-test-execution-dependencies.csv','mime':'text/csv;charset=utf-8','content':(ROOT/'docs/development/first-product-test-execution-dependencies.csv').read_text()}
    svg = (ROOT / 'docs/development/first-product-test-layers.svg').read_text()
    task_rows = list(csv.DictReader((ROOT/'docs/development/first-product-test-enablement.csv').open()))
    tasks = ''.join('<tr>'+''.join('<td>'+esc(row[key])+'</td>' for key in ['task_id','gate','title','owners','depends_on','acceptance','status'])+'</tr>' for row in task_rows)
    sources = ''.join(f'<li><code>{esc(s["path"])}</code><br><span class="hash">SHA-256 {esc(s["sha256"])}</span></li>' for s in plan['sources'])
    style = '''
:root{color-scheme:dark;font:15px/1.8 -apple-system,BlinkMacSystemFont,"PingFang SC","Hiragino Sans",sans-serif;color:#e9e9ef;background:#141720}*{box-sizing:border-box}body{margin:0;background:radial-gradient(ellipse at 90% 0,#29394a60,transparent 28%),#141720}main,header,footer{width:min(1440px,100%);margin:auto;padding:28px 40px}header{padding-top:52px}h1{font-size:39px;line-height:1.35;font-weight:600;letter-spacing:-.8px;margin:12px 0}h2{font-size:25px;line-height:1.5;margin:0 0 18px}h3{font-size:19px;line-height:1.5;margin:9px 0}h4{font-size:14px;font-weight:600;margin:15px 0 8px}p{margin:8px 0;color:#b8bdc9}a{color:#d1d4de;text-underline-offset:4px}button,input,select{font:inherit;color:inherit}button{cursor:pointer}button,input,select{border:1px solid #b1b6c82b;border-radius:8px;background:#282d39;padding:8px 12px}button:hover{background:#3b414e}button:focus-visible,a:focus-visible,summary:focus-visible,input:focus-visible,select:focus-visible{outline:2px solid #c0c6d6;outline-offset:3px}summary{cursor:pointer;list-style:none}summary::-webkit-details-marker{display:none}summary:after{content:'＋';color:#969eaf;float:right}details[open]>summary:after{content:'−'}nav,.actions{display:flex;gap:12px 24px;flex-wrap:wrap;margin:23px 0 10px}nav a{font-size:13px}.eyebrow{font-size:11px;color:#969eaf;letter-spacing:1px}.lead{max-width:950px;font-size:17px}.metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin:25px 0}.metrics div{background:#34394550;border:1px solid #a0a0ad24;border-radius:13px;padding:15px 20px}.metrics b{font-size:27px;display:block;line-height:1.4}.metrics span{color:#aab1bf;font-size:12px}.chapter{margin:0 0 26px;padding:28px;background:#222632d9;border:1px solid #080b12;border-radius:16px;box-shadow:inset 0 1px 0 #ffffff0b,0 9px 32px #0002;min-width:0;scroll-margin-top:20px}.section-lead{margin-top:-8px;margin-bottom:20px}.features{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;align-items:start}.feature-card{background:#30364360;border:1px solid #a0a0ad24;border-radius:12px;overflow:hidden}.feature-card>summary{padding:18px 20px}.feature-card>summary p{font-size:13px;line-height:1.7;min-height:44px}.disclosure{display:block;color:#a4adbe;font-size:12px;margin-top:13px}.feature-body{padding:0 20px 20px;border-top:1px solid #a0a0ad17}.feature-body li{font-size:13px}.feature-body button{width:100%;font-size:12px;margin-top:15px}.filters{display:grid;grid-template-columns:2fr 1.7fr 1.5fr 1.2fr;gap:12px}.filters label{font-size:12px;color:#aab1bf;display:grid;gap:5px}.filters input,.filters select{width:100%;min-width:0;background:#181c27}.resultbar{display:flex;gap:12px 20px;justify-content:space-between;align-items:center;flex-wrap:wrap;margin:19px 0}.resultbar p{margin:0;font-size:13px}.resultbar button{font-size:12px;padding:5px 11px}.test-case{border:1px solid #a0a0ad24;border-radius:11px;margin:12px 0;overflow:hidden;background:#2c324044}.test-case>summary{padding:17px 21px}.case-top{display:flex;gap:13px;align-items:center;flex-wrap:wrap;margin-bottom:6px}.case-top code{font-size:12px;color:#d1d5e0;letter-spacing:.1px}.badge,.status{border:1px solid #a0a0ad28;border-radius:5px;padding:1px 8px;font-size:11px;color:#aab1bf;background:#9ea5b609}.status{margin-left:auto}.test-case summary strong{display:block;font-size:16px;font-weight:550}.case-sub{display:block;font-size:12px;color:#8f99ab;margin-top:7px}.case-body{border-top:1px solid #a0a0ad24;padding:20px}.source{font-size:11px;overflow-wrap:anywhere}.contract{display:flex;gap:7px 20px;flex-wrap:wrap;font-size:12px;color:#b7bfce}.columns{display:grid;grid-template-columns:1fr 1.2fr 1.2fr;gap:25px}.columns.two{grid-template-columns:1fr 1fr}.columns section{min-width:0}.columns p,li{font-size:13px;overflow-wrap:anywhere}ul,ol{padding-left:21px;margin:7px 0}li{margin:7px 0;color:#b8bdc9}.note{border-left:2px solid #929caf;padding:4px 0 4px 13px;font-size:12px;color:#9ca6b8;margin-top:16px}.note ul{padding-left:16px}.note li{font-size:12px}.variant{border-bottom:1px solid #a0a0ad24;padding:10px 0;font-size:11px}.variant summary{display:flex;gap:7px 12px;flex-wrap:wrap}.variant summary span{color:#8e99ab}.variant li{font-size:11px}.variant code{overflow-wrap:anywhere}pre{max-width:100%;overflow-x:auto;white-space:pre;font-size:12px;padding:14px;border:1px solid #a0a0ad24;border-radius:8px;background:#14172270}code{font-family:ui-monospace,SFMono-Regular,Consolas,monospace}.diagram-scroll{overflow-x:auto;border-radius:12px;border:1px solid #a0a0ad24;margin-top:20px}.diagram-scroll svg{display:block;width:100%;min-width:800px;height:auto}.release-grid{display:grid;grid-template-columns:1fr 1fr;gap:22px}.table-wrap{overflow:auto}table{border-collapse:collapse;font-size:12px;width:100%}th,td{padding:10px 12px;border:1px solid #a0a0ad24;text-align:left;vertical-align:top;min-width:100px;max-width:420px;overflow-wrap:anywhere}th{color:#d5d9e3;background:#9ca5b40d;white-space:nowrap}td{color:#abb4c5}.hash{font-size:11px;color:#8793a8;word-break:break-all}.muted{color:#8995a9}footer{font-size:12px;color:#8995a9;padding-bottom:60px}[hidden]{display:none!important}.empty{padding:40px;text-align:center}.sources details{margin-top:16px}.sources li{font-size:12px}@media(max-width:1100px){.features{grid-template-columns:repeat(2,1fr)}.filters{grid-template-columns:1fr 1fr}.columns{grid-template-columns:1fr}.columns.two{grid-template-columns:1fr 1fr}}@media(max-width:650px){main,header,footer{padding:20px 16px}header{padding-top:32px}h1{font-size:29px}h2{font-size:22px}.chapter{padding:20px 15px}.features,.release-grid,.filters,.columns.two{grid-template-columns:1fr}.metrics{grid-template-columns:1fr 1fr;gap:9px}.metrics div{padding:12px 15px}.case-body{padding:16px}.test-case>summary{padding:16px}.case-top{gap:7px}.status{margin-left:0}.diagram-scroll svg{min-width:800px}}@media print{body{background:#fff;color:#111}.chapter,.feature-card,.test-case{background:white;color:#111;box-shadow:none}.filters,.actions,.resultbar button,nav{display:none}p,li,td{color:#333}details{break-inside:avoid}details>*{display:block!important}.diagram-scroll{overflow:visible}.diagram-scroll svg{min-width:0}.features{grid-template-columns:1fr 1fr}main,header{padding:0;width:100%}.columns{display:block}}
'''
    style += '.prep-toolbar{display:flex;align-items:center;gap:20px;flex-wrap:wrap;margin:18px 0;color:#aab1bf;font-size:13px}.prep-toolbar select{margin-left:9px}.small{font-size:11px;margin-top:8px;padding:5px 8px}td strong{display:block;color:#c5cbd6;font-size:13px}td details p{font-size:12px}#prerequisite-table td:nth-child(3){min-width:310px;max-width:600px}.maintenance-tree{overflow:auto;background:#161b26;border-radius:9px;padding:16px 20px;margin:18px 0}.maintenance-tree pre{font:12px/1.9 ui-monospace,SFMono-Regular,monospace;margin:0;color:#b8c1d0}#maintenance-table code{font-size:11px;overflow-wrap:anywhere}.filters{grid-template-columns:2fr 1.5fr 1.3fr 1.1fr 1.5fr}@media(max-width:1100px){.filters{grid-template-columns:1fr 1fr}}@media(max-width:650px){.filters{grid-template-columns:1fr}}'
    script = ''' 
const caseNodes=[...document.querySelectorAll('.test-case')], search=document.getElementById('search'), feature=document.getElementById('feature'), layer=document.getElementById('layer'), variant=document.getElementById('variant'), prerequisite=document.getElementById('prerequisite');
function filter(){const query=search.value.trim().toLocaleLowerCase();let visible=0, combos=0;for(const node of caseNodes){const vs=node.dataset.variants.split(' '),dependencies=JSON.parse(node.dataset.dependencyVariants);const selected=vs.filter(v=>(!variant.value||variant.value===v)&&(!prerequisite.value||(dependencies[prerequisite.value]||[]).includes(v)));const match=(!query||node.dataset.search.includes(query))&&(!feature.value||node.dataset.features.split(' ').includes(feature.value))&&(!layer.value||node.dataset.layer===layer.value)&&selected.length>0;node.hidden=!match;node.querySelectorAll('.variant').forEach(item=>item.hidden=!selected.includes(item.dataset.variant));if(match){visible++;combos+=selected.length;}}document.getElementById('count').textContent=visible+' / '+caseNodes.length+' 个用例定义 · '+combos+' 个执行组合';document.getElementById('empty').hidden=visible>0;}
for(const field of [search,feature,layer,variant,prerequisite])field.addEventListener('input',filter);
document.getElementById('reset').addEventListener('click',()=>{for(const field of [search,feature,layer,variant,prerequisite])field.value='';filter();});
document.querySelectorAll('[data-select-feature]').forEach(button=>button.addEventListener('click',()=>{search.value='';layer.value='';variant.value='';prerequisite.value='';feature.value=button.dataset.selectFeature;filter();document.getElementById('cases').scrollIntoView({behavior:'smooth'});}));
document.getElementById('expand').addEventListener('click',()=>{for(const node of caseNodes)if(!node.hidden)node.open=true;});document.getElementById('collapse').addEventListener('click',()=>{for(const node of caseNodes)node.open=false;});
document.querySelectorAll('[data-select-prereq]').forEach(button=>button.addEventListener('click',()=>{for(const field of [search,feature,layer,variant])field.value='';prerequisite.value=button.dataset.selectPrereq;filter();document.getElementById('cases').scrollIntoView({behavior:'smooth'});}));
const preparationRows=[...document.querySelectorAll('#prerequisite-table tbody tr')];document.getElementById('prereq-provider').addEventListener('input',event=>{let visible=0;for(const row of preparationRows){row.hidden=event.target.value==='user'?row.dataset.provider==='team':event.target.value==='team'?row.dataset.provider!=='team':false;if(!row.hidden)visible++;}document.getElementById('prep-count').textContent=visible+' / '+preparationRows.length+' 项';});
const downloads=JSON.parse(document.getElementById('download-data').textContent);document.querySelectorAll('[data-download]').forEach(button=>button.addEventListener('click',()=>{const data=downloads[button.dataset.download],blob=new Blob([data.content],{type:data.mime}),url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=data.name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}));filter();
'''
    document=f'''<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Luna · 首版功能测试用例与验证方式</title><style>{style}</style></head><body><header><div class="eyebrow">LUNA · FIRST PRODUCT TEST PLAN · 2026.10.19</div><h1>首版功能，逐项如何验证。</h1><p class="lead">按功能查看验证路径，再展开每个用例的前置、操作步骤、预期结果、运行环境与证据。此报告是测试规划，全部产品用例尚未执行。新增前置供给责任、测试源码维护位置与逐组合依赖。</p><div class="metrics"><div><b>12</b><span>首版用户功能</span></div><div><b>{count}</b><span>用例定义 · 97继承 + 31机制</span></div><div><b>{executions}</b><span>逐环境执行组合</span></div><div><b>9 层</b><span>底层规则至正式包与发布</span></div></div><nav><a href="#storage-enforcement">存储验收与字段边界</a><a href="#prerequisites">测试前要准备什么</a><a href="#maintenance">测试维护在哪里</a><a href="#features">按功能看验证方式</a><a href="#cases">展开具体测试用例</a><a href="#path">分层验证路径</a><a href="#release">发布通过条件</a><a href="#enablement">测试设施任务</a></nav><div class="actions"><button data-download="storage">下载存储子验收登记</button><button data-download="operations">下载前置与维护登记</button><button data-download="dependencies">下载153组合依赖映射</button><button data-download="executions">下载153项执行工作表</button><button data-download="coverage">下载需求追溯表</button><button data-download="plan">下载完整机器登记JSON</button></div></header><main>{storage_html}{operations_html}<section class="chapter" id="features"><h2>每项功能，计划怎么验</h2><p class="section-lead">展开功能卡片可看验证路径和通过判断；“查看关联用例”会筛选下方详细步骤。同一用例可支撑多个功能，关联数量不相加作为覆盖率。</p><div class="features">{''.join(feature_cards)}</div></section><section class="chapter" id="cases"><h2>具体测试用例</h2><p class="section-lead">每条可展开；环境组合也可逐项展开。自动化规则、可控故障、真实服务、原生窗口及实际游戏保留不同证据，不能互相替代。</p><div class="filters"><label>搜索用例、步骤或模块<input id="search" type="search" placeholder="例如：关联、取消、SQLite、TP-27"></label><label>功能<select id="feature"><option value="">全部首版功能</option>{feature_options}</select></label><label>层级<select id="layer"><option value="">全部九层</option>{layer_options}</select></label><label>执行变体<select id="variant"><option value="">全部环境组合</option>{variant_options}</select></label><label>前置准备<select id="prerequisite"><option value="">全部准备依赖</option>{prerequisite_options}</select></label></div><div class="resultbar"><p id="count" aria-live="polite"></p><div><button id="reset">清除筛选</button> <button id="expand">展开筛选结果</button> <button id="collapse">收起全部</button></div></div><div id="empty" class="empty" hidden>没有匹配用例，请调整筛选。</div><div id="case-list">{''.join(cases)}</div></section><section class="chapter" id="path"><h2>先验证边界，再验证同一正式包</h2><p>按垂直切片进入各层，L1/L2/L5可按冻结契约并行。最终RC的原生旅程、四游戏、质量与分发全部收齐后再评审上线。</p><div class="diagram-scroll" tabindex="0" role="region" aria-label="九层测试路径图，可横向滚动">{svg}</div></section><section class="chapter" id="release"><h2>什么情况下才能上线</h2><div class="release-grid"><section><h4>必须齐全</h4>{items(['完整设计最终批准，B0资源/中文辞书/许可及性能/语言质量政策先冻结','全部153个冻结组合有独立执行回执并通过；无必需not_run、blocked或failed','正式包旅程全部绑定同一最终RC及实际安装bundle；低层绑定同源构建','SR/MO/Never7/サクラノ詩逐款原文、翻译、阅读学习和连续样本；SR按中文版，排除Ever17','零发布阻断缺陷，真实capture/runtime adapter、分发安装和迁移恢复证据齐全'])}</section><section><h4>当前真实状态</h4>{items(['本报告128个定义/153个组合全部未执行，没有产品pass',operations_check_text,'完整设计待最终评审，指标/语言质量/资源前置仍待B0冻结','真实产品capture/runtime adapter尚未实现，19项旧架构债务未迁移，发布继续阻断','本轮没有修改产品源码、运行游戏或模型，也没有提交推送'])}</section></div></section><section class="chapter" id="enablement"><h2>怎样把这份计划变成可运行的测试</h2><p>以下12项测试设施任务全部planned，随B0～B5研发切片实现。TE-10必须把153个case@variant组合接入真实运行与发布adapter；现有97平面来源校验不能代替它。</p><div class="table-wrap"><table><thead><tr><th>任务</th><th>门禁</th><th>设施</th><th>主责</th><th>前置任务</th><th>怎样验收设施</th><th>状态</th></tr></thead><tbody>{tasks}</tbody></table></div></section><section class="chapter sources"><h2>来源与结果边界</h2><p>计划身份：{esc(plan['plan_id'])}。完整PRD范围为FP-01～12，模型配置AI-CFG-01～16。原111条CR完整保留，首版使用其中55条；另有22条MC与20条GL。CR-10-VISUAL按当前first-product-v1最终批准稿显式替代旧尺寸定义。</p><p>macOS arm64 + CrossOver是本次声明范围，实际Rust PE32/64均须验证；不外推Windows原机、Mac Intel或原生macOS游戏取字。静态来源hash不能证明实际运行或内容质量。</p><details><summary>查看原验收来源指纹</summary><ul>{sources}</ul></details><p class="hash">机器登记 SHA-256 {hashlib.sha256(plan_path.read_bytes()).hexdigest()}</p></section></main><footer>自包含离线HTML：筛选、展开和CSV/JSON下载无需网络。测试定义与结果分开，当前所有产品组合均NOT_RUN。保留原项目License和验收来源。</footer><script id="download-data" type="application/json">{inline_json(downloads)}</script><script>{script}</script></body></html>'''
    OUTPUT.parent.mkdir(exist_ok=True)
    OUTPUT.write_text(document)
    print(json.dumps(dict(html=str(OUTPUT.relative_to(ROOT)),bytes=OUTPUT.stat().st_size,cases=count,executions=executions,product_tests_executed=False),ensure_ascii=False))


if __name__=='__main__':
    main()
