#!/usr/bin/env python3
"""Package the first usable product's high-fidelity design and verified captures."""
import hashlib
import html
import json
from pathlib import Path
import shutil
import zipfile

REPORT_STYLE = """
:root{font-family:-apple-system,BlinkMacSystemFont,'PingFang SC',sans-serif;color:#e9e9ef;color-scheme:dark;background:#111521}*{box-sizing:border-box}body{max-width:1240px;margin:auto;padding:40px 28px 64px}body:before{content:"";position:fixed;inset:0;z-index:-1;background:linear-gradient(#101522bb,#101522dd),var(--report-wallpaper) center/cover no-repeat}h1{font-size:34px;font-weight:600;letter-spacing:-.8px;margin:12px 0}h2{font-size:23px;font-weight:600;letter-spacing:-.3px}p{line-height:1.8;color:#b0b2c0}a{color:#c4c5cf;text-underline-offset:3px}.tag{font-size:12px;letter-spacing:.5px;color:#9495a5}nav{display:flex;gap:18px;flex-wrap:wrap;line-height:1.7;margin:24px 0 30px}section{background:rgba(35,38,49,.8965);backdrop-filter:blur(25px) saturate(105%);border:1px solid rgba(7,9,17,.88);border-radius:16px;padding:24px;margin:24px 0;box-shadow:0 9px 32px #0003,inset 0 1px 0 #ffffff12}img{display:block;max-width:100%;height:auto;margin:20px auto 0;border-radius:10px}figure{margin:0}figcaption{color:#b0b2c0}.grid{display:grid;grid-template-columns:1fr 1fr;gap:18px}.grid section{margin:0;padding:18px}small{color:#9495a5}.steps{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin:25px 0}.steps span{padding:10px 15px;border-radius:7px;background:rgba(58,58,68,.863);color:#e9e9ef}.foot{font-size:13px}.jump{display:flex;gap:12px;flex-wrap:wrap}@media(max-width:700px){body{padding:24px 16px}.grid{grid-template-columns:1fr}h1{font-size:28px}section{padding:16px}}
"""


def page(title, body, wallpaper="assets/wallpaper-frame.png"):
    return f'<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{html.escape(title)}</title><style>{REPORT_STYLE}</style><body style="--report-wallpaper:url(\'{html.escape(wallpaper)}\')">{body}</body></html>\n'


ROOT = Path(__file__).resolve().parents[2]
DESIGN = ROOT / "docs/design/first-product-v1"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    qa = json.loads((DESIGN / "previews/verification.json").read_text())
    if not qa["artifact_qa_passed"]:
        raise ValueError("Browser design checks must pass before packaging")
    for name, digest in qa["source_sha256"].items():
        if sha(DESIGN / name) != digest:
            raise ValueError(f"Source changed since capture: {name}")
    shots = qa["captures"]
    nav = '<nav><a href="style-guide.md">已确认风格与视觉规范</a><a href="index.html">进入完整交互稿</a><a href="entry-flow.html">按使用旅程评审</a><a href="comparison.html">与原稿对照</a><a href="../../product/first-product-scope.md">首版范围</a><a href="coverage.csv">12项功能覆盖</a><a href="../../product/game-library-window-prd.md">添加游戏与窗口关联PRD</a><a href="previews/verification.json">浏览器检查证据</a></nav>'
    body = '<div class="tag">LUNA · FIRST USABLE PRODUCT · 高保真设计提案</div><h1>从游戏里的原句，走到理解它。</h1><p>首版包含游戏取文、翻译、内置分词与词典、按需AI精读、模型配置。游戏与独立字幕、设置、学习窗各自保留状态，让每一步都能连续操作。</p>' + nav
    body += '<div class="steps"><span>添加游戏或选择窗口</span>→<span>关联并接入，再选正文</span>→<span>游戏上阅读</span>→<span>点词与精读</span>→<span>记录与恢复</span></div><p class="foot">19个游戏画面/原文配对来自此前本机Core采集；译文、词法和解释为人工设计数据。图片不证明新Rust正式包通过，当前原型不调用真实服务。</p>'
    for shot in shots:
        image = "previews/" + shot["file"]
        body += f'<section id="{html.escape(Path(shot["file"]).stem)}"><h2>{html.escape(shot["title"])}</h2><p>{html.escape(shot.get("description", ""))}</p><a href="{image}"><img loading="lazy" src="{image}" alt="{html.escape(shot["title"])}"></a></section>'
    body += f'<p class="foot">{len(shots)}张真实浏览器截图；{len(qa["checks"])}组设计交互检查通过，0 HTTP请求、0控制台错误。案例风格已确认，整稿页面待用户评审，产品运行与模型质量仍为NOT_RUN。</p>'
    (DESIGN / "gallery.html").write_text(page("Luna · 首版高保真设计画廊", body))

    groups = [
        ("首次入库与窗口接入", ["38-library-empty.png", "39-add-game-entry.png", "40-add-game-review.png", "41-window-picker.png", "42-window-mapping.png", "43-window-connected-reading.png"]),
        ("同目标复用、切换与未知目标", ["44-window-binding.png", "48-window-switch-confirm.png", "45-window-unbound.png", "46-custom-batch-review.png", "47-unknown-target-check.png"]),
        ("准备服务并连接游戏", ["01-first-open.png", "02-configure-translation.png", "03-connect-target.png", "05-connect-check.png", "06-stream-selection.png"]),
        ("在游戏画面上持续阅读", ["07-source-before-translation.png", "08-live-reading.png", "19-hud-menu.png"]),
        ("点词、配置并精读固定原句", ["10-local-word-card.png", "11-learning-unconfigured.png", "12-settings-over-reading.png", "13-return-fixed-sentence.png", "15-learning-result.png"]),
        ("回看、修正与恢复", ["20-history.png", "21-history-save-failure.png", "25-manual-translation.png", "28-integrity-recovery.png"]),
        ("设置返回主界面，保留编辑与阅读", ["37-settings-return-main.png"]),
    ]
    flow = '<div class="tag">完整使用旅程 · 独立窗口与同一份数据</div><h1>每一步都从产品入口走过去。</h1><p>打开交互稿即可连续操作。场景参数只用于评审直达截图，不替代连接、配置、应用或开始分析。</p>' + nav
    by_name = {shot["file"]: shot for shot in shots}
    for title, names in groups:
        flow += f'<h2>{html.escape(title)}</h2>'
        for name in names:
            if name not in by_name:
                raise ValueError(f"Required journey capture is missing: {name}")
            shot = by_name[name]
            flow += f'<section><h3>{html.escape(shot["title"])}</h3><img loading="lazy" src="previews/{name}" alt="{html.escape(shot["title"])}"></section>'
    (DESIGN / "entry-flow.html").write_text(page("Luna · 首版使用旅程", flow))

    comparisons = [
        ("阅读浮窗", "SCR-01-light.jpg", "08-live-reading.png", "保留原文/译文层级、32pt安静操作行，材质统一为已确认暗色案例；舒适尺寸沿用此前v2提案，紧凑仍保留原500/16/22基线。"),
        ("翻译服务设置", "SCR-15-light.jpg", "23-connection-entities.png", "保留十类单层侧栏、分组Form与中性选中，视觉统一为已确认案例。新稿将连接/模型/用途和草稿/应用版本拆成真实可操作的数据关系。"),
        ("历史记录", "SCR-08-light.jpg", "20-history.png", "继承列表/详情结构及原文译文的对应关系；补足重译来源、人工修改和保存失败的连续流程。"),
    ]
    comparison = '<div class="tag">已确认材质案例 · 首版统一设计</div><h1>把同一套磨砂材质，带进每一个窗口。</h1><p>暗蓝灰连续窗体、较厚的中性灰内面、同族灰色线性图标、中性控件与细暗边界，均来自用户确认的独立案例。这里比较共同的视觉语言，页面内容不同，不将像素差异当作一致性评分。</p>' + nav
    approved = DESIGN / "assets/reference-approved-material.png"
    before = DESIGN / "assets/reference-full-product-before-restyle.png"
    reference_records = [{"file": str(approved.relative_to(DESIGN)), "source": "docs/design/macos-material-case-v1/previews/case-window.png", "sha256": sha(approved), "approval_scope": "user-approved first-product visual style; no runtime acceptance"}, {"file": str(before.relative_to(DESIGN)), "source": "first-product-v1 draft before approved case style application", "sha256": sha(before), "approval_scope": "historical unapproved draft"}]
    comparison += '<section id="window-material"><h2>已确认案例与游戏库</h2><p>整窗连续磨砂，侧栏与页头共享底层光影，内容区不再切成独立白底。</p><div class="grid"><figure><figcaption>用户确认的材质案例</figcaption><img src="assets/reference-approved-material.png" alt="已确认暗色磨砂案例"></figure><figure><figcaption>首版游戏库 · 统一后</figcaption><img src="previews/35-library-material-dark.png" alt="统一材质游戏库"></figure></div></section>'
    comparison += '<section id="icons-color"><h2>图标与控件：去掉额外的装饰配色</h2><p>分类和工具图标统一为同一灰色线性SVG，不再使用彩色方块或Unicode符号；主按钮改为深灰底白字，选中与链接采用中性亮度。</p><div class="grid"><figure><figcaption>修正前 · 设置分类带独立色块</figcaption><img loading="lazy" src="assets/reference-icons-color-before-settings.png" alt="修正前彩色设置图标"></figure><figure><figcaption>修正后 · 统一灰色线性图标</figcaption><img loading="lazy" src="previews/23-connection-entities.png" alt="修正后设置图标"></figure><figure><figcaption>修正前 · 浅紫主按钮</figcaption><img loading="lazy" src="assets/reference-icons-color-before-library.png" alt="修正前主按钮配色"></figure><figure><figcaption>修正后 · 中性控件层级</figcaption><img loading="lazy" src="previews/35-library-material-dark.png" alt="修正后主按钮配色"></figure></div></section>'
    comparison += '<section><h2>同一风格的不同窗口</h2><div class="grid"><figure><figcaption>服务连接与模型设置</figcaption><img loading="lazy" src="previews/23-connection-entities.png" alt="统一材质设置"></figure><figure><figcaption>独立精读与游戏阅读</figcaption><img loading="lazy" src="previews/15-learning-result.png" alt="统一材质精读"></figure><figure><figcaption>字幕原文、译文与注音</figcaption><img loading="lazy" src="previews/09-hud-detail.png" alt="统一材质字幕"></figure><figure><figcaption>原文旁的内置词典</figcaption><img loading="lazy" src="previews/10-local-word-card.png" alt="统一材质词卡"></figure></div></section>'
    comparison += '<section><h2>游戏库修改前后</h2><div class="grid"><figure><figcaption>此前导航透色稿 · 已被接替</figcaption><img loading="lazy" src="assets/reference-full-product-before-restyle.png" alt="统一前主窗"></figure><figure><figcaption>案例风格统一后</figcaption><img loading="lazy" src="previews/35-library-material-dark.png" alt="统一后主窗"></figure></div></section>'
    comparison += '<section><h2>可选浅色与系统减少透明度</h2><div class="grid"><figure><figcaption>同系列浅色适配</figcaption><img loading="lazy" src="previews/34-library-material-light.png" alt="同系列浅色"></figure><figure><figcaption>减少透明度回到同色系实底</figcaption><img loading="lazy" src="previews/36-library-material-reduced.png" alt="同色系实色回退"></figure></div><p class="foot">当前默认采用已确认暗色。HTML表达材质视觉，原生窗口与正式产品仍按计划分别验证。</p></section>'
    for title, original_name, new_name, explanation in comparisons:
        original = ROOT / "docs/product/original/design/previews" / original_name
        reference = DESIGN / "assets" / ("reference-" + original_name)
        shutil.copy2(original, reference)
        reference_records.append({"file": str(reference.relative_to(DESIGN)), "source": str(original.relative_to(ROOT)), "sha256": sha(reference), "approval_scope": "inherited aligned-v1 visual baseline; no new runtime acceptance"})
        comparison += f'<section><h2>{title}</h2><p>{explanation}</p><div class="grid"><figure><figcaption>原设计基线</figcaption><img loading="lazy" src="assets/{reference.name}" alt="原稿{title}"></figure><figure><figcaption>首版高保真提案</figcaption><img loading="lazy" src="previews/{new_name}" alt="新稿{title}"></figure></div></section>'
    (DESIGN / "comparison.html").write_text(page("Luna · 首版与原稿视觉对照", comparison))
    (DESIGN / "reference-manifest.json").write_text(json.dumps(reference_records, ensure_ascii=False, indent=2) + "\n")
    inventory = []
    for file in sorted(DESIGN.rglob("*")):
        if file.is_file() and file != DESIGN / "manifest.json":
            inventory.append({"file": str(file.relative_to(DESIGN)), "bytes": file.stat().st_size, "sha256": sha(file)})
    manifest = {"revision": "first-product-v1-unified-bind-connect", "material_baseline": "user-approved macos-material-case-v1", "default_theme": "dark", "scope": "FP-01 through FP-12; user confirmed first usable product", "user_review": "full_layout_pending_review", "product_runtime_tests": "NOT_RUN", "real_model_executed": False, "live_game_executed": False, "game_scene_source": "assets/manifest.json", "visual_references": "reference-manifest.json", "design_qa": "previews/verification.json", "files": inventory}
    (DESIGN / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")

    destination = ROOT / "reports/first-product-high-fidelity-20261018.zip"
    prefix = "luna-first-product-high-fidelity/"
    landing = '<div class="tag">LUNA · 首个可用产品版 · 离线高保真评审包</div><h1>游戏取文、翻译和学习，放在同一条旅程里。</h1><p>解压后直接打开，不需要开发服务器或模型账号。</p><nav><a href="docs/design/first-product-v1/index.html">进入可交互整稿</a><a href="docs/design/first-product-v1/gallery.html">完整截图画廊</a><a href="docs/design/first-product-v1/entry-flow.html">按旅程评审</a><a href="docs/design/first-product-v1/comparison.html">与原稿对照</a></nav><p class="foot">素材来自此前本机游戏采集，译文/词法/精读为人工设计样本。没有运行游戏或调用真实模型；案例风格已确认、整稿布局待评审，正式产品回归NOT_RUN。附PRD源文档快照，其指向完整工程其他资料的链接需回项目查看。</p>'
    with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr(prefix + "index.html", page("Luna · 首版高保真离线包", landing, "docs/design/first-product-v1/assets/wallpaper-frame.png"))
        for file in sorted(DESIGN.rglob("*")):
            if file.is_file():
                archive.write(file, prefix + str(file.relative_to(ROOT)))
        case = ROOT / "docs/design/macos-material-case-v1"
        for file in sorted(case.rglob("*")):
            if file.is_file():
                archive.write(file, prefix + str(file.relative_to(ROOT)))
        for name in ["first-product-scope.md", "model-configuration-prd.md", "ai-reading-prd.md", "model-configuration-acceptance.csv", "game-library-window-prd.md", "game-library-window-acceptance.csv"]:
            file = ROOT / "docs/product" / name
            archive.write(file, prefix + str(file.relative_to(ROOT)))
    print(json.dumps({"zip": str(destination.relative_to(ROOT)), "bytes": destination.stat().st_size, "sha256": sha(destination), "captures": len(shots), "design_files": len(inventory) + 1}, ensure_ascii=False))


if __name__ == "__main__":
    main()
