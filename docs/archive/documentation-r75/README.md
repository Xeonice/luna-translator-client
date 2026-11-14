# Luna Translator：产品设计与取文验证

**当前处于设计阶段。首版完整客户端研发暂停；本轮只整理 PRD、高保真稿与交互流程，不提交或推送代码。**

当前产品规范以[PRD 索引](docs/product/README.md)与[首版范围](docs/product/first-product-scope.md)为准。首版包含游戏取文、翻译、内置日语分词词典、按需 AI 精读和模型配置；原 56 项需求及 100 文件快照保留。

本轮确定的[首版研发架构与技术基线](docs/architecture/first-product-engineering-baseline.md)覆盖功能、框架技术栈、模块职责、组件复用和设计规范；配套[目标目录与文件组织](docs/architecture/engineering-layout.md)、[16个模块矩阵](docs/architecture/engineering-modules.csv)和[34个组件族](docs/architecture/engineering-components.csv)。本轮仅形成方案，不继续客户端研发。

[打开本轮架构 HTML 报告](reports/first-product-engineering-20261018/index.html) · [下载离线 ZIP](reports/first-product-engineering-20261018.zip)。

后续迭代遵循[执行与一致性规则](docs/development/iteration-governance.md)，编码前填写[迭代卡](docs/development/iteration-template.md)，按[基线指纹与确认范围](docs/development/iteration-baseline.json)追溯；协作者入口见[AGENTS.md](AGENTS.md)。自动检查、人工评审及正式包实测分别记录，架构确认不代表本轮开始产品实现。

已接入[OpenSpec与可执行门禁](docs/development/engineering-gates.md)：`npm run verify`统一执行规范映射、PRD/设计/技术锁、源码范围、TS AST/Cargo依赖边界、正反例及实际编译。根依赖只服务工程工具；产品仍未按新架构迁移，完整发布仍被阻断。

**唯一完整设计入口：[高保真整稿](docs/design/first-product-v1/index.html)** · [截图画廊](docs/design/first-product-v1/gallery.html) · [使用旅程](docs/design/first-product-v1/entry-flow.html) · [离线 ZIP](reports/first-product-high-fidelity-20261018.zip)。选择窗口并核对后，一次确认“关联并接入”即启动 Hook 接入；不再沿用独立关联后另点连接的旧流程。

视觉沿用用户确认的[macOS 磨砂案例](docs/design/macos-material-case-v1/index.html)：连续暗蓝灰材质、中性线性图标、克制配色和轻返回入口。整稿最终评审与真实产品回归分别记录，产品回归保持 `NOT_RUN`。

旧字幕与模型配置专题已收敛到[历史归档](docs/design/archive/README.md)，不再作为当前设计入口。设计所用游戏画面与原文来自此前采集，译文、分词、释义及 AI 分析均为离线设计样本。

已有技术基础为独立精简 Core 与可打包的 Rust/Tauri 原文客户端，两者尚未完成集成。[Reader 0.1.3 的关联并接入验证](docs/development/desktop-client.md#关联并接入版-013)保留为技术切片证据，不能代替首版完整产品交付。日语学习与模型的数据边界见[架构方案](docs/architecture/japanese-study-and-llm.md)。

## 入口

- [原 PRD 与设计来源](docs/product/README.md)
- [首版完整高保真设计](docs/design/first-product-v1/index.html) · [12项功能覆盖](docs/design/first-product-v1/coverage.csv)
- [模型配置产品需求](docs/product/model-configuration-prd.md) · [AI 阅读与学习主线](docs/product/ai-reading-prd.md)
- [当前模型配置设计](docs/design/first-product-v1/index.html?scenario=settings) · [整稿截图](docs/design/first-product-v1/gallery.html)
- [原设计画廊](docs/product/original/design/index.html)：22 屏、89 张原设计图。
- [客户端重建架构](docs/architecture/client-rebuild-architecture.md) · [ADR-003](docs/architecture/ADR-003-client-rebuild-on-core.md)
- [最新重建与回归离线 HTML 报告](reports/client-rebuild-plan-20261018-language-study/index.html) · [ZIP](reports/client-rebuild-plan-20261018-language-study.zip)
- [开发里程碑](docs/development/plan.md) · [可执行任务 CSV](docs/development/client-rebuild-tasks.csv)
- [产品回归计划](docs/development/client-regression.md) · [用例 CSV](docs/development/client-regression.csv)
- [旧架构文档](docs/architecture/development-architecture.md) · [旧 HTML 快照](reports/luna-development-architecture-20261018/index.html)
- [旧客户端已实现的 JSONL v1](docs/architecture/hook-api-v1.md)，Core 专用 v2 尚待实现。
- [本次构建、测试与运行证据](docs/development/verification.md)
- [56 项需求到新工作包的映射](docs/development/requirements.csv)
- [固定的上游原生依赖](native/upstream.lock.json)

## 边界

`hook-contracts` 定义共享协议；`lunahook-host` 是 Windows x64 Rust 无视图宿主，加载原版 LunaHost DLL，
把原文和状态回调转换为事件。它不包含 Python、Qt、WebView、翻译服务或历史数据库。

独立 [luna-hooks-core](https://github.com/Xeonice/luna-hooks-core) 已在源码层裁剪为只取文的 Hook/Host/Injector，并保留 GPL 与第三方许可。
其 ABI 1 使用七回调，不能直接替换当前十回调运行时。后续先做双 broker、资源锁与正式包原文回归，再做翻译/HUD/配置/历史闭环。

`hook-client` 管理独立宿主、目标身份与原文快照；`apps/desktop` 是 Tauri 2 + React/TypeScript/Vite 桌面窗口，
可选择游戏与完整文本流、实时显示原文和最近历史。翻译、上下文和持久化仍属后续应用核心。
macOS 的 CrossOver 路线通过指定 bottle 运行 Windows 宿主；原生 macOS 游戏是另一个取字后端。

## 本机检查

```sh
cargo test --workspace
cargo check --workspace --target x86_64-pc-windows-gnu
python3 -m unittest discover -s tests -v
python3 scripts/check_project_plan.py
```

准备原生资源（只校验、提取，不执行程序）：

```sh
python3 scripts/prepare_lunahook_runtime.py \
  --archive /absolute/path/LunaTranslator_x64.zip \
  --output runtime/lunahook-v12.1.2.3
```

在 Windows x64 开发环境构建宿主后，以 `--runtime-dir` 指向该资源目录。生产使用优先 MSVC 工具链；
GNU 目标编译检查用于发现 Windows 平台代码错误，不能替代真实 Windows/Wine 执行。
非 Windows 运行应明确返回平台不支持，不生成模拟连接或模拟文本。

当前真实游戏的对照结果在 `diagnostics/`：原版 SR x86 取文与 Never 7 x64 MAGES2 在线翻译已跑通。
这些是**原版对照证据**。新宿主的真实注入、连续回调与退出恢复独立验收，状态见开发计划。

旧宿主已构建为Windows x64程序，并在CrossOver中独立通过SR x86的真实原文接收。
新 Core 另由原生探针验证 SR、MO、Never7、サクラノ詩 10th，见其[脱敏摘要](../luna-hooks-core/verification/published-summary.json)。
这些证据不能作为重建客户端的产品回归结果；最终包四游戏、反复重连与长稳需重新验收。
此前交付的HTML架构报告为取文验证前的v1.0快照，最新运行状态以该验证记录为准。

## 打包可视化客户端

```sh
python3 scripts/build_desktop.py
```

当前产物为 `dist/Luna Reader.app` 和 `dist/Luna-Reader-0.1.3-macos-arm64.zip`；此前版本的ZIP保留供原验证报告追溯。构建与实机结果以[0.1.3 验证状态](docs/development/desktop-client.md#关联并接入版-013)为准。
这是现有原版 LunaHost 运行时上的关联并接入测试包，新 Core 的接入状态未因本次修改改变。
应用包内含前端、Windows Rust 宿主、身份探针及固定原版 Hook 资源；运行时使用本机已装 CrossOver，
无需 Python、开发服务器或从仓库目录启动。打开应用后选择游戏窗口，核对目标并确认“关联并接入”，
客户端自动启动 Hook 接入；收到候选流后选取对白，游戏推进时原文持续更新。进程列表仍可作为选择目标的入口。具体边界见[桌面客户端说明](docs/development/desktop-client.md)。

0.1.3 将窗口确认改为“关联并接入”：同一目标的活动会话复用，不重复注入；不同目标先明确确认切换；接入失败保留已核验窗口并提供重试。解除窗口关联仍保留取文会话。0.1.2 的[历史独立关联验证](docs/development/desktop-client.md#历史窗口关联测试版-012)保留其原始结论。

0.1.1 将进程发现改为查询所选 bottle 内可核验身份的普通 Windows 进程，不再限定游戏文件名。
《サクラノ詩 10th Anniversary edition》x64 已完成5条真实原文的窗口显示验证，推荐选择 `Artemis` 流，
见[实机记录](docs/development/sakura10th-verification.md)及[离线图文报告](reports/sakura10th-validation-20261018.html)。

## 生成开发架构报告

本轮重建规划的离线报告使用：

```sh
python3 scripts/build_client_rebuild_report.py
python3 scripts/check_client_rebuild_plan.py
node scripts/report/verify_client_rebuild_report.cjs reports/client-rebuild-plan-20261018-language-study .build/client-rebuild-report-language-qa
```

报告可直接离线打开，附 55 项开发任务、111 条未执行回归用例、两项补充验收及原/新设计画廊。这里的校验仅针对规划与报告，不是产品实机回归。

以下为历史大型架构报告的生成方式，内容不作为新 Core 接入规范。

报告正文来自[完整架构文档](docs/architecture/development-architecture.md)，图表和样式离线内置；ZIP保留原稿、设计图、当前源代码与选定的无目标验证证据。

```sh
python3 -m pip install --target .build/report-tools -r scripts/report/requirements.txt
python3 scripts/build_architecture_report.py --output reports/architecture-new-version
```

生成器拒绝覆盖已有目录和ZIP。可使用本机Chrome检查导出报告，检查只打开报告文件：

```sh
npm install --prefix .build/report-browser --no-save --package-lock=false playwright-core@1.56.1
node scripts/report/verify_report.cjs reports/architecture-new-version reports/architecture-new-version-qa
```

非默认Chrome路径可用 `REPORT_BROWSER_EXECUTABLE` 指定浏览器可执行文件。报告工具依赖不属于产品运行时依赖。


## 首版逐层测试与上线准入

[测试规划](docs/development/first-product-test-plan.md)按规范/领域/协议/基础设施/应用/视觉/正式包/四游戏/质量分发九层验证，附[153项执行组合](docs/development/first-product-test-executions.csv)、[追溯表](docs/development/first-product-test-coverage.csv)和[测试设施任务](docs/development/first-product-test-enablement.csv)。所有产品用例仍未执行。

```sh
npm run verify:test-plan
python3 scripts/quality/render_test_plan.py --check
npm run verify
```

以上只检查计划、工程约束及编译，不运行产品/游戏/模型；真实产品adapter与正式包回归按计划后续实施。
