# LunaTranslator 当前设计

`aligned-v1` 原生 Mac 视觉基线已于 2026-10-13 确认，对应 PRD 1.1 的 60 张截图。按 [apple-design](../.agents/skills/apple-design/SKILL.md) 制作的上轮 `ocr-source-v1` 15 张稿保留追溯；本轮 `ocr-selection-v2` 新增 6 张原生截图，改为快捷键唤起屏幕选区、拖框松手开始。用户于 2026-10-13 确认本轮 6 张选区视觉，单独记录为 **accepted（visual only）**；上轮 15 张仍保留原评审状态。2026-10-16新增CrossOver/Wine的SCR-03/04/22 `compat-x86-v1` 8张真实原生窗口样张，已视觉检查，用户状态为not_reviewed，未捕获兼容状态继续planned。

生产客户端计划采用Tauri 2＋React/TypeScript＋Vite，在Windows和macOS共用页面、HUD、选区、组件、主题tokens及command绑定。本目录SwiftUI/AppKit源仍作为已确认设计的可编辑基线载体保留，布局、文案、交互、截图、批准范围和hash不因框架调整而改变；它不是另一套待维护的生产产品UI。新共享WebView与Windows实际视觉/辅助技术仍需独立验证，不能继承这些原生截图的accepted状态；新视觉稿按下文revision规则另存和复核。

直接进入 [屏幕框选评审](index.html?screen=SCR-06&variant=ocr-selection-dragging-light)，按顶部“快捷键唤起 → 拖框选区 → 实时翻译”查看，另有任务、设置与调区入口。画廊保留旧 60 张已确认稿和上轮 OCR 草稿，当前 6 张选区稿已单独获得视觉确认。

## CrossOver x86 兼容稿 · compat-x86-v1

从[连接检查](index.html?screen=SCR-04&variant=compat-x86-check-light)进入，可切换缺少32位组件、错误组合和取消；另有[游戏配置](index.html?screen=SCR-03&variant=compat-x86-config-light)及[能力报告](index.html?screen=SCR-22&variant=compat-x86-report-light)。固定“告别回忆 · memo01 / Game.exe”为设计示例，画面中的环境/路径/架构是编排数据，不能当真实检测结果。Ever17修复版不作为原发行GC验收资源；样张不证明memo01或其他x86游戏接入成功。

8张稿复用原生状态块、键值行、按钮及高级诊断折叠，普通流程显示“自动匹配32位游戏”，不新增位数选择或DLL安装步骤。helper64/Host64与同源injector32/Hook32的名称、版本及hash留在高级诊断/报告；启动、组件就绪、连接、取文、浮窗、回写独立呈现。资源缺失/错误组合注入前阻止并可修复重试；取消保留游戏/bottle，注入后无清理回执时必须显示目标内状态未确认。

这次范围仅可编辑设计及规格，不修改产品运行实现。`compat-x86-v1`为visual_checked / **not_reviewed**，runtimeTests为NOT_RUN。旧60张与OCR21张的图片、hash及用户确认范围保留。[QA记录](compat-x86-v1-qa.json)列出实际样张、构建、查看范围和限制；尚未捕获首次bottle确认、资源修复进度、注入中取消细分、目标退出、200%字号和完整键盘/VoiceOver。

```sh
cd design
./build.sh
python3 capture-compat-previews.py --revision-tag compat-x86-v2 --dry-run
python3 capture-compat-previews.py --revision-tag compat-x86-v2
```

本次最终样张位于`previews/compat-x86-v1-final-r3/`。捕获只启动自己的原生预览；本机`screencapture`写中文路径失败，包装器先在唯一ASCII临时目录捕获同一窗口，再复制到已有防覆盖发布流程。它没有读取真实游戏画面、运行helper或发起注入。再次导出使用新tag，不覆盖现有版本。

## 本轮 OCR 流程

- **SCR-06 屏幕选区**：主要入口为 ⌥⌘O 或“框选实时 OCR 区域…”菜单，在原前台应用所在屏幕直接呈现准星；矩形外暗化、内保留原画，拖动显示边框与尺寸，新建松手即开始持续识别，无需先打开主窗口。Esc 取消保留原任务；已有范围调整使用 Return 应用或 Esc 恢复，键盘支持 1pt 精调。选区图示单位为设计坐标 pt，生产捕获由平台按缩放换算像素。
- **SCR-06 任务工作页**：用于查看/配置/恢复；固定暂停、继续、停止和查看浮窗。屏幕缩略预览只读，重新框选和调整跳到屏幕选择层；应用窗口绑定是次级入口。浮窗按选区边界避让，屏幕无足够外部空间时选择较少相交位置，也可拖动微调。
- **SCR-14 OCR 引擎**：使用原生 Form 配置引擎、识别语言和周期/图像稳定/输入事件策略，高级参数按需展开。开始/停止任务在工作页；OCR 图像处理和翻译文字处理分别说明。
- **SCR-01 阅读浮窗**：普通网页背景上的识别原文与译文，保留两个安静的暂停/继续和更多入口。更多菜单包含保持置顶、隐藏浮窗、调整 OCR 范围、停止实时 OCR。设计中的显隐快捷键为 ⌥⌘H；预览只响应应用内按键，全局注册尚未实现。隐藏与暂停是不同操作。

本轮 6 张稿包含准星待拖框、拖框中、调整已有范围、松手开始后的浮窗，以及任务页浅深色。上轮 15 张保留为上一版本，包括窗口绑定、权限/目标失效、OCR 设置和阅读状态。多范围独立运行、在线服务细节、图像/事件高级参数、处理中与失败细分、原地翻译、更多菜单展开、隐藏恢复、真实多显示器和 200% OCR 阅读等仍待出图与运行验证；完整契约见 [设计规格](../docs/02-design-spec.md)。

## Apple-design 复核

**复核限定为设计视觉和源代码，不代表运行验收。** 这是一款供游戏玩家及普通应用使用者持续阅读译文的 Mac 工具。用户本轮反馈指出工作页缩略图内的选取不符合期望，因此将选择移动到实际屏幕语境：准星、外暗内亮的矩形、松手开始，浮窗承载阅读，配置留在任务与设置页。这一流程以用户需求为依据；[CleanShot 官方功能说明](https://cleanshot.com/features?xs=1)也提供屏幕区域选择作为操作参考。

本轮复核为 **Good**，已修复固定浮窗位置会遮挡任意选区的问题。浮窗按实际高度与范围避让；近全屏选区没有外部空间时只能最少重叠，可拖动微调，不承诺所有选区始终零重叠。最新样张 HUD 的 13pt 来源标签对比为 4.92:1，原文/译文为 6.02:1；选择提示与尺寸标签分别为 19.08:1 / 19.44:1。此处数值来自本轮 PNG 的局部像素，完整辅助功能审计未执行。

- `pointing-devices.md › Best practices`：“Be consistent when responding to mouse and trackpad gestures.” 采用屏幕拖出矩形的熟悉动作，新建松手提交；调整已有任务时保留取消与应用。
- `keyboards.md › Custom keyboard shortcuts`：“Define custom keyboard shortcuts for only the most frequently used app-specific commands.” 框选 ⌥⌘O 与浮窗显隐 ⌥⌘H 分开，生产支持自定义及注册失败备用菜单；预览仅应用内响应。

- `layout.md › Visual hierarchy`：“Use progressive disclosure to make layouts cleaner and easier to interact with.” 引擎参数集中设置页，高级触发参数折叠；主任务动作不随正文滚动。
- `privacy.md › Requesting permission`：“Request permission only when your app clearly needs access to the data or resource.” 权限说明出现在捕获任务中；拒绝或目标失效保留范围并提供恢复入口。预览不请求真实权限。
- `menus.md › Toggled items`：“Consider using a checkmark to show that an attribute is currently in effect.” 置顶使用原生勾选项，显隐动作明确命名，暂停持续显示文字和继续符号。
- `accessibility.md › Vision`：“Strive to meet color contrast minimum standards.” 修复了浅色网页上 HUD 的低对比及工作页 13pt 次级文字。修改前次级文字为 3.95:1，现浅色 6.01:1、深色 7.01:1；HUD 13pt 来源标签从 2.83:1 提升至 5.12:1，16pt 原文从 3.08:1 提升至 6.09:1。数值来自当前 PNG 满覆盖字形及邻域背景，限定这些样张，不能推断所有系统、背景与辅助外观。

四个基础颜色角色保持系统语义；以下为本机样张的 sRGB 解析值，代码不硬编码系统 accent：

| 角色 | 浅色参考值 | 深色参考值 | 对其表面的对比 |
| --- | --- | --- | --- |
| 内容表面 | #FFFFFF | #1E1E1E | 文字承载面 |
| 主要文字（primary） | #272727 | #DDDDDD | 14.94:1 / 12.27:1 |
| 次级文字（primary × 0.72） | #636363 | #A8A8A8 | 6.01:1 / 7.01:1 |
| 选区/主要动作（system accent） | #007AFF | #007AFF | 对上述表面约 4.02:1 / 4.15:1；用作选区边框和原生控件，状态另有文字 |

文字使用系统字体：标题 23pt、正文/状态 13pt，HUD 原文 16pt、译文 22pt、来源 13pt；工具符号 16pt Medium，HUD 命中区域 32×32pt、间隔 8pt。OCR HUD 的 hudWindow 加 50% 黑色遮罩，增加对比时 65%，减少透明度时用纯黑底。只在浮动阅读层使用半透明。预览没有必需动画，刷新和状态变化用文字表达；完整辅助外观仍待运行验证。

主要选区模式不含主窗壳，尺寸标签与提示不压住所选文字；次级任务页仍采用原生布局：

```text
屏幕选区（设计画布1200×750pt）
┌──────────────────────────────────┐
│       拖动框选 · 松手开始 · Esc   │
│       当前前台应用原画            │
│   暗化 ┌─准星拖出的亮区────┐ 暗化 │
│        └──────────────────┘ 尺寸 │
└──────────────────────────────────┘

1000×720pt                    900×600pt
┌侧栏┬实时 OCR───────────┐     ┌侧栏┬实时 OCR────────────┐
│    │状态  开始/暂停/停止│     │    │状态  固定主要操作  │
│    │捕获方式 / 目标窗口 │     │    │方式 / 目标 / 范围  │
│    │内容预览 + 识别边框 │     │    │预览 + 可编辑边框   │
│    │范围动作 / 设置摘要 │     │    │次级设置与结果滚动  │
└────┴───────────────────┘     └────┴────────────────────┘
```

## 浏览与编辑

从仓库根目录启动统一评审入口：

```sh
python3 -m http.server 4211 --bind 127.0.0.1
```

访问 <http://127.0.0.1:4211/design/index.html?screen=SCR-06&variant=ocr-selection-dragging-light>。原生可编辑预览需要 macOS 26+、Apple Silicon 与 Xcode Command Line Tools：

```sh
cd design
./build.sh
open .build/LunaAlignedDesign.app
```

菜单“输入 → 框选实时 OCR 区域…”或应用内 ⌥⌘O 进入选择演示，“状态”切换准星/拖框/调区/识别。拖框松手更新本地范围并显示示例浮窗。背景是自绘桌面，选择场景仅占据自己的预览窗口；未安装真正的全屏覆盖层或后台全局热键，也未捕获 Safari。参数只保存在内存中，原文/译文为演示数据。

重建并捕获一份**新的**选区与任务版本，先预检目标路径，再打开自己的原生预览实例：

```sh
./build.sh
python3 capture-ocr-previews.py --revision selection --revision-tag ocr-selection-v3 --dry-run
python3 capture-ocr-previews.py --revision selection --revision-tag ocr-selection-v3
```

示例tag由本次变更选择；输出保存为`previews/ocr-selection-v3/<原场景文件名>`。不提供tag时，默认目标指向本轮已确认的6张图，脚本会在打开任何预览前拒绝执行。三份metadata中任一份的accepted路径均受到保护；已有tag文件也不会被覆盖，应使用新tag。整批先预检，实际PNG完成后用不替换已有文件的原子操作保存，避免局部覆盖后才发现后续目标已确认。

捕获脚本只启动自己的预览实例，读取自有窗口的PID/window ID，保存实际窗口PNG并结束该实例；它不截图用户选取的其他应用或执行OCR。`--revision selection`的默认集合仅含6张选区/任务状态，`--revision source-v1`含上轮15种工作页/设置/阅读状态。快捷键或菜单变化需另捕获实际受影响的设置、菜单和提示，不能凭默认6张图宣布所有变更已可评审。

新稿登记使用动态revision：

1. 实际导出并查看图片，向`manifest.json`受影响页面加入`ocr-*`预览key、新路径和实际SHA-256，更新总数。
2. 在`manifest.json`、`screen-plan.json`、`verification.json`三处添加同名`ocr…DesignExtension`字段，例如`ocrShortcutDesignExtension`；填写同一`version: ocr-shortcut-v3`、captures、captureCount、screens及状态。新稿初始`designAcceptance: not_reviewed`，查看后可记录`visualReview: visual_checked`；`runtimeTests`仍为`NOT_RUN`。
3. 新稿不继承旧批准字段。只有取得实际用户反馈后，才登记批准日期、feedback来源/原文、明确capture范围及其批准SHA；旧版本的文件、批准hash和历史sourceHashes保留。更新顶层`verification.sourceHashes`表示当前可编辑源，历史extension内sourceHashes仍表示当时版本。
4. 运行`python3 ../docs/verify-spec.py`。检查器从同组`ocr…DesignExtension`字段发现revision，核对三处注册表、实际manifest数量与hash、批准范围及反馈；增加合法新revision不需要为名称/数量/批准日期修改检查器代码。

完整变更仍执行[研发流程](../docs/06-development-workflow.md)的四面检查、后置核验与独立评审；动态注册不意味着任何新稿自动获得视觉确认。

| 文件 | 职责 |
| --- | --- |
| [DesignSystem.swift](DesignSystem.swift) | 页面清单、语义样式、符号控件 |
| [OCRSourcePreview.swift](OCRSourcePreview.swift) | OCR 工作页、范围编辑和普通应用阅读场景 |
| [OCRSelectionPreview.swift](OCRSelectionPreview.swift) | 准星、屏幕遮罩拖框、调区取消与浮窗避让 |
| [OverlaySample.swift](OverlaySample.swift) | HUD、固定操作区、阅读文字与更多菜单 |
| [CoreScreens.swift](CoreScreens.swift) | 核心页面及实时 OCR 工作页入口 |
| [CompatConnectionPreview.swift](CompatConnectionPreview.swift) | CrossOver x86配置、连接检查/恢复和能力报告可编辑稿 |
| [capture-compat-previews.py](capture-compat-previews.py) | 沿用自有窗口捕获流程导出独立兼容revision |
| [SettingsScreens.swift](SettingsScreens.swift) | 10 类设置，包括引擎/语言/持续识别策略 |
| [main.swift](main.swift) | 原生窗口、侧栏、sheet、菜单与样张选择 |
| [capture-ocr-previews.py](capture-ocr-previews.py) | 自有原生预览窗口捕获 |
| [manifest.json](manifest.json) | 89 张实际截图、版本/状态说明与校验值；新8张兼容稿单独待评审 |
| [screen-plan.json](screen-plan.json) | 页面、需求、计划变体与已捕获图态 |
| [verification.json](verification.json) | 基线验收、新稿视觉复核、未执行项与源文件校验值 |

素材由 `assets/` 提供，[署名说明](assets/README.md)记录来源。本轮普通网页/字幕内容为设计绘制的示例画面，不是 Safari 实时捕获。

## 验收状态

旧 60 张基线 accepted；上轮 15 张 OCR 稿保留追溯，本轮 6 张屏幕选区稿于 2026-10-13 获用户视觉确认；兼容环境扩展8张稿visual_checked / not_reviewed，其余未捕获状态planned。全量键盘与 VoiceOver、200% 字号、辅助外观、多显示器、Spaces、全局热键和真实应用叠加尚未执行完整审计。

原生预览是设计载体，不决定生产 UI 框架。真实屏幕捕获、OCR、翻译服务、系统 pin 与后台全局快捷键未实现或验证；本轮截图不能作为 D1/D2 的运行证据。[49 个客户端运行用例](../docs/04-test-plan.md)仍均为 **NOT_RUN**。


D1实验边界（2026-10-17用户决定）：同一游戏仅本项目独占取文，并发注入未支持；检测到已有外部Host仍拒绝接管。此项是运行/证据前提，未新增产品UI视觉，compat-x86-v1样张和旧批准范围保持原hash，不记为新视觉确认。
