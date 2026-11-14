"""Documentation-only SVG of the target repository layout."""
from html import escape


def directory_map_svg():
    parts = ['''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 1520" role="img" aria-labelledby="directory-title directory-desc">
<title id="directory-title">Luna 目标工程目录结构图</title>
<desc id="directory-desc">客户端仓库包含桌面应用、Rust 模块、资源、测试工具、文档和本地产物。实线表示目录包含关系。底部独立的 luna-hooks-core 仓库仅通过锁定构建产物接入 native，不属于客户端子目录。全图为目标规划，不表示已实现。</desc>
<defs><marker id="dir-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M0 0L10 5L0 10z" fill="#a4a8b8"/></marker></defs>
<style>text{font-family:-apple-system,BlinkMacSystemFont,'PingFang SC',sans-serif;fill:#e9e9ef}.mono{font-family:'SFMono-Regular',Consolas,monospace;font-size:16px}.label{font-size:17px;font-weight:550}.detail{font-size:14px;fill:#b0b2c0}.small{font-size:12px;fill:#a4a8b8}.node{fill:#292d39;stroke:#555b6c;stroke-opacity:.55}.edge{fill:none;stroke:#747b8e;stroke-width:1.5}.sub{fill:#202430;stroke:#4c5365;stroke-opacity:.55}</style>
<rect width="1200" height="1520" rx="16" fill="#1a1e29"/>
<rect x="24" y="24" width="1152" height="96" rx="12" class="node"/>
<text x="48" y="61" class="label">luna-translator/</text>
<text x="48" y="91" class="detail">目标工程结构 · 按切片逐步落地，不是当前实现清单</text>
<text x="1152" y="61" class="small" text-anchor="end">实线 = 目录层级</text>
<text x="1152" y="91" class="small" text-anchor="end">Cargo.toml / Cargo.lock · 固定工具链</text>
<path d="M54 120V1248" class="edge"/>
''']

    def box(x, y, width, height, title, lines, subtitle=None, sub=False):
        parts.append(f'<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="10" class="{"sub" if sub else "node"}"/>')
        parts.append(f'<text x="{x+20}" y="{y+29}" class="mono">{escape(title)}</text>')
        offset = 55
        if subtitle:
            parts.append(f'<text x="{x+20}" y="{y+offset}" class="small">{escape(subtitle)}</text>')
            offset += 25
        for line in lines:
            parts.append(f'<text x="{x+20}" y="{y+offset}" class="detail">{escape(line)}</text>')
            offset += 25

    def branch(y, centers):
        parts.append(f'<path d="M54 {y}H80' + ''.join(f'M80 {y}V{cy}H{x}' for x, cy in centers) + '" class="edge"/>')

    branch(230, [(100, 230)])
    box(100, 148, 1076, 184, 'apps/desktop/', ['React + TypeScript', 'Tauri 桌面入口与组件测试'])
    parts.append('<path d="M308 230H330M330 230V240H352M330 230V240H766" class="edge"/>')
    box(352, 166, 394, 148, 'src/', ['app · bridge · windows', 'features · ui · i18n', '四类窗口、业务页面、统一设计规范'], sub=True)
    box(766, 166, 390, 148, 'src-tauri/', ['src/ipc · src/platform', 'capabilities · permissions', '系统窗口、装配和权限边界'], sub=True)

    branch(496, [(100, 496)])
    box(100, 356, 1076, 280, 'crates/', [], 'Rust 模块 · 领域、I/O、采集与语言进程分别负责')
    cards = [
        ('app-core/', '计划新增 · 业务权威', ['domain · application · ports', 'contracts · projections']),
        ('app-infra/', '计划新增 · 具体 I/O', ['storage · providers · credentials', 'language · dictionary · hook backend']),
        ('language-worker/', '计划新增 · 本机独立进程', ['MeCab FFI · UniDic · alignment', '词法处理与资源限制']),
        ('hook-contracts/', '已有模块 · 计划演进协议', ['broker DTO · 协议版本']),
        ('hook-client/', '已有模块 · 计划适配新 Core', ['目标核验 · 进程监督 · 资源']),
        ('lunahook-host/', '已有 Rust Windows broker', ['FFI · 回调复制 · 注入 · JSONL']),
    ]
    for index, (name, subtitle, lines) in enumerate(cards):
        x, y = 120 + (index % 3) * 348, 432 + (index // 3) * 101
        box(x, y, 328, 89, name, lines[:1], subtitle, sub=True)
        # The second detail remains in the complete textual tree below the map.

    branch(731, [(100, 731), (648, 731)])
    box(100, 660, 528, 142, 'native/', ['core-runtime.lock.json（计划）', '旧 upstream.lock.json 仅作迁移对照', '锁 Core 产物、架构、哈希与许可'])
    box(648, 660, 528, 142, 'resources/', ['language/ · language-resources.lock.json', 'MeCab · UniDic · 内置词典 · licenses/', '固定资源集合；随包离线使用'])

    branch(897, [(100, 897), (466, 897), (832, 897)])
    box(100, 826, 344, 142, 'schemas/', ['计划生成的 IPC/schema 快照', '业务 / broker / 语言协议版本', 'Rust DTO → TypeScript'])
    box(466, 826, 344, 142, 'tests/', ['contracts/ · integration/ · fixtures/', '桌面视觉测试另在 desktop/tests/', '兼容、行为与真实产品回归'])
    box(832, 826, 344, 142, 'scripts/', ['build_desktop.py · resources/', 'contracts/ · report/', '构建、资源校验、契约与报告'])

    branch(1063, [(100, 1063), (832, 1063)])
    box(100, 992, 710, 142, 'docs/', ['product/ · design/first-product-v1/', 'architecture/ · development/ · design/archive/', '当前 PRD、设计、架构决策、计划与历史归档'])
    box(832, 992, 344, 142, 'reports/', ['可分享 HTML 与离线 ZIP', '文档与可视化证据', '不属于产品运行时'])

    branch(1222, [(100, 1222), (466, 1222), (832, 1222)])
    box(100, 1158, 344, 128, 'diagnostics/', ['本地实机证据', '默认不进入分享包'])
    box(466, 1158, 344, 128, '.build/', ['工具、下载与临时缓存', '不作为产品源码'])
    box(832, 1158, 344, 128, 'dist/', ['正式客户端包与产物', '不作为产品源码'])

    parts.append('''<path d="M24 1320H1176" stroke="#747b8e" stroke-opacity=".4" stroke-dasharray="5 7"/>
<text x="24" y="1352" class="small">独立仓库 · 不在上方客户端目录树内</text>''')
    box(24, 1370, 438, 120, 'luna-hooks-core/', ['C++ Hook / Host / Injector · ABI', '独立构建、版本、原始 License 与归属'])
    parts.append('''<path d="M480 1430H734" class="edge" stroke-dasharray="5 6" marker-end="url(#dir-arrow)"/>
<text x="606" y="1408" class="small" text-anchor="middle">锁定发行产物 / manifest</text>''')
    box(752, 1370, 424, 120, '客户端 native/ 资源入口', ['供 Rust broker 加载 Core DLL 与注入器', '不复制 Core 源码到 app-core/'])
    parts.append('</svg>')
    return ''.join(parts)
