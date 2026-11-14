#!/usr/bin/env python3
"""Publish reviewed design candidates without replacing approved reference images."""
import argparse
import hashlib
import html
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[2]
DESIGN = ROOT / 'docs/design/first-product-v1'
BEFORE = ROOT / '.build/design-six-gaps-20261019/before'
STYLE = '''html{color-scheme:dark}*{box-sizing:border-box}body{margin:0;background:#151925;color:#e9e9ef;font:15px/1.7 -apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif}main{max-width:1240px;padding:44px 28px 70px;margin:auto}h1{font-size:32px;letter-spacing:-.8px;font-weight:600}h2{font-size:22px;font-weight:600}p,small{color:#b0b2c0}a{color:#c4c5cf;text-underline-offset:4px}nav,.links{display:flex;gap:18px;flex-wrap:wrap}section{margin:24px 0;padding:24px;border:1px solid #ffffff13;border-radius:16px;background:#ffffff06;box-shadow:inset 0 1px #ffffff08}img{max-width:100%;height:auto;display:block;border-radius:12px;margin-top:18px}.tag{font-size:12px;color:#a4a6b5}.grid{display:grid;grid-template-columns:1fr 1fr;gap:16px}.grid section{margin:0}code{overflow-wrap:anywhere}@media(max-width:760px){main{padding:24px 16px}.grid{grid-template-columns:1fr}h1{font-size:27px}}'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_qa(directory):
    path = directory / 'verification.json'
    data = json.loads(path.read_text())
    if data.get('artifact_qa_passed') is not True:
        raise ValueError(f'Design QA has not passed: {path}')
    if data.get('console_errors') or data.get('http_requests'):
        raise ValueError(f'Design must be offline and free of browser errors: {path}')
    for name, expected in data['source_sha256'].items():
        file = DESIGN / name
        if not file.is_file() or digest(file) != expected:
            raise ValueError(f'Design changed after QA: {name}')
    for shot in data['captures']:
        source = directory / shot['file']
        if not source.is_file() or source.resolve().parent != directory.resolve():
            raise ValueError(f'Invalid or missing direct candidate capture: {source}')
    return data


def reserve(base):
    for n in range(1, 1000):
        path = base if n == 1 else base.with_name(base.name + f'-attempt-{n:02d}')
        try:
            path.mkdir(parents=True, exist_ok=False)
            return path
        except FileExistsError:
            pass
    raise ValueError('Too many candidate attempts')


def page(title, content):
    return '<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>' + html.escape(title) + '</title><style>' + STYLE + '</style><body><main>' + content + '</main></body></html>\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--qa', type=Path, required=True)
    parser.add_argument('--legacy-qa', type=Path, required=True)
    args = parser.parse_args()
    qa, legacy = read_qa(args.qa), read_qa(args.legacy_qa)
    destination = reserve(DESIGN / 'candidates/interaction-gaps-20261019')
    for label, directory, data in [('six-gaps', args.qa, qa), ('existing-journeys', args.legacy_qa, legacy)]:
        sub = destination / label
        sub.mkdir()
        for shot in data['captures']:
            shutil.copy2(directory / shot['file'], sub / shot['file'])
        shutil.copy2(directory / 'verification.json', sub / 'verification.json')
    relative = destination.relative_to(DESIGN).as_posix()
    cards = [
        ('运行窗口预览', 'window-preview', 'window-preview', '主动开启、窗口身份、帧时间、暂停、权限与失效；现有游戏画面为历史配对样本回放。'),
        ('浮窗移动和快捷键', 'reading', 'hud-controls', '拖动手柄换位置；常用操作直接在工具栏，更多只留低频项且不重复。网页内快捷键不证明系统全局注册和多屏恢复。'),
        ('自定义Select', 'components', 'select-showcase', '真正展开中性磨砂菜单、选中勾记、方向键、取消、禁用与长名称；不使用浏览器原生弹出菜单。'),
        ('会话完整内容', 'session-texts', 'session-history', '正文历史、全部提取与Hook日志分开；所有来源、分页、跟随、搜索与缺口状态清楚。'),
        ('半透明、分词和注音', 'hud-ruby', 'ruby-material', '对照纯文本、仅注音、分词/POS、未知读音和失败、系统减少透明度。真实worker与原生材质仍待实施验收。'),
        ('组件和按钮一致性', 'components', 'select-showcase', '沿原确认材质和SVG；轻返回、普通/悬停/焦点/禁用/选中及展开态逐项评审。')
    ]
    content = '<span class="tag">LUNA · 六项交互修订 · 设计候选待评审</span><h1>把缺失的操作补进完整旅程。</h1><p>本轮更新PRD与可交互设计。没有修改真实客户端；下列截图是浏览器对候选原型的实际渲染，不是当前正式包截图。游戏素材为既有实机配对画面的设计回放。</p><nav><a href="index.html">进入完整交互稿</a><a href="../../product/reading-window-session-prd.md">阅读专项PRD</a><a href="../../product/reading-window-session-acceptance.csv">34项待实施验收</a><a href="../../development/design-review-gap-analysis.md">门禁为什么没有报出差异</a></nav><div class="grid">'
    for title, scenario, anchor, detail in cards:
        content += '<section><h2>' + html.escape(title) + '</h2><p>' + html.escape(detail) + '</p><a href="index.html?scenario=' + scenario + '#' + anchor + '">打开此处交互 →</a></section>'
    content += '</div><section><h2>沿用已确认材质</h2><p>左侧是已确认案例，右侧是新增字幕候选。结构与操作有意变化，不宣称两张不同条件的画面零差异；原参考图片不改。</p><div class="grid"><figure><figcaption>原确认磨砂案例</figcaption><img src="assets/reference-approved-material.png" alt="已确认磨砂案例原图"></figure><figure><figcaption>新增字幕候选</figcaption><img src="' + relative + '/six-gaps/21-hud-translucent.png" alt="半透明字幕设计候选"></figure></div></section><section><h2>候选截图与实际设计检查</h2><p>新检查 ' + str(len(qa['checks'])) + ' 组、' + str(len(qa['captures'])) + ' 张截图；既有旅程 ' + str(len(legacy['checks'])) + ' 组、' + str(len(legacy['captures'])) + ' 张截图。两类均无HTTP请求/控制台错误。它们不执行真实游戏、原生全局快捷键、实时采屏或真实词法worker。</p><div class="links"><a href="' + relative + '/six-gaps/verification.json">六项设计检查回执</a><a href="' + relative + '/existing-journeys/verification.json">既有旅程检查回执</a></div></section>'
    for shot in qa['captures']:
        url = relative + '/six-gaps/' + shot['file']
        content += '<section id="' + html.escape(Path(shot['file']).stem) + '"><h2>' + html.escape(shot['title']) + '</h2><p>' + html.escape(shot.get('description', '')) + '</p><a href="' + html.escape(url) + '"><img loading="lazy" src="' + html.escape(url) + '" alt="' + html.escape(shot['title']) + '"></a></section>'
    content += '<section><h2>评审和实现状态</h2><p>原确认材质、PNG、token和acceptance-v3参照保持原字节，R16不刷新。新稿与PRD已改变，原51/51与旧正式包不能沿用为本稿通过。34条RW以及153产品组合仍not_run。设计检查通过不等于新增产品功能已实现。</p><p><a href="comparison.html">查看原稿对照</a> · <a href="gallery.html">查看既有完整画廊</a></p></section>'
    (DESIGN / 'interaction-review.html').write_text(page('Luna · 六项交互设计修订', content))
    # Preserve every original image/reference and the old gallery body; add navigation only.
    old_gallery = (BEFORE / 'docs/design/first-product-v1/gallery.html').read_text()
    notice = '<section><h2>本轮六项修订候选</h2><p>窗口预览、可移动HUD、自定义Select、完整会话、注音与材质、组件状态已补入。下面原画廊是修订前的设计截图，保持历史身份。</p><a href="interaction-review.html">先查看本轮交互与候选截图 →</a></section>'
    (DESIGN / 'gallery.html').write_text(old_gallery.replace('</nav>', '</nav>' + notice, 1))
    old_flow = (BEFORE / 'docs/design/first-product-v1/entry-flow.html').read_text()
    (DESIGN / 'entry-flow.html').write_text(old_flow.replace('</nav>', '</nav>' + notice, 1))
    old_comparison = (BEFORE / 'docs/design/first-product-v1/comparison.html').read_text()
    (DESIGN / 'comparison.html').write_text(old_comparison.replace('</nav>', '</nav>' + notice, 1))
    manifest = json.loads((BEFORE / 'docs/design/first-product-v1/manifest.json').read_text())
    manifest.update({'revision':'first-product-v1-six-interaction-gaps-candidate','user_review':'full_layout_pending_review','interaction_revision_review':'design_candidate_pending_review','design_qa':relative+'/six-gaps/verification.json','existing_journey_qa':relative+'/existing-journeys/verification.json','review_entry':'interaction-review.html','product_runtime_tests':'NOT_RUN','real_model_executed':False,'live_game_executed':False,'real_tokenizer_executed':False,'native_global_shortcuts_executed':False,'baseline_policy':'R16 retained; design/PRD drift awaits review; no approved reference or token replacement'})
    manifest['files'] = [{'file':file.relative_to(DESIGN).as_posix(),'bytes':file.stat().st_size,'sha256':digest(file)} for file in sorted(DESIGN.rglob('*')) if file.is_file() and file != DESIGN / 'manifest.json']
    (DESIGN/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
    archive = ROOT / 'reports/luna-first-product-design-six-gaps-20261019.zip'
    counter = 2
    while archive.exists():
        archive = ROOT / f'reports/luna-first-product-design-six-gaps-20261019-attempt-{counter:02d}.zip'
        counter += 1
    archive.parent.mkdir(exist_ok=True)
    prefix = 'luna-six-gaps-design/'
    with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as z:
        z.writestr(prefix+'index.html',page('Luna · 六项设计修订','<h1>Luna · 六项设计修订</h1><nav><a href="docs/design/first-product-v1/interaction-review.html">从六项修订开始评审</a><a href="docs/design/first-product-v1/index.html">完整交互稿</a></nav><p>解压后直接打开；设计回放，不读取系统窗口或调用模型。</p>'))
        for directory in [DESIGN,ROOT/'docs/design/macos-material-case-v1']:
            for file in sorted(directory.rglob('*')):
                if file.is_file():
                    z.write(file,prefix+file.relative_to(ROOT).as_posix())
        for name in ['README.md','first-product-scope.md','model-configuration-prd.md','ai-reading-prd.md','game-library-window-prd.md','reading-window-session-prd.md','reading-window-session-acceptance.csv','game-library-window-acceptance.csv','model-configuration-acceptance.csv']:
            file=ROOT/'docs/product'/name
            z.write(file,prefix+file.relative_to(ROOT).as_posix())
        for file in [ROOT/'docs/development/design-review-gap-analysis.md',ROOT/'LICENSE']:
            z.write(file,prefix+file.relative_to(ROOT).as_posix())
    receipt={'status':'passed_design_package_only','entry':str((DESIGN/'interaction-review.html').relative_to(ROOT)),'candidate_directory':str(destination.relative_to(ROOT)),'zip':str(archive.relative_to(ROOT)),'zip_sha256':digest(archive),'zip_bytes':archive.stat().st_size,'design_checks':len(qa['checks']),'design_captures':len(qa['captures']),'existing_journey_checks':len(legacy['checks']),'existing_journey_captures':len(legacy['captures']),'product_implementation_executed':False,'approved_references_changed':False}
    out=ROOT/'.build/design-six-gaps-20261019/package.json'
    out.write_text(json.dumps(receipt,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(receipt,ensure_ascii=False,indent=2))


if __name__=='__main__':
    main()
