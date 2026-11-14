# 运行窗口、HUD与会话测试覆盖契约

日期：2026-10-19。用户已明确本轮设计 review 无问题；该确认只覆盖刚评审的 PRD／交互设计及 Select 间距修订，不提升产品实现、原生能力、真实词法或发布验收。本轮实施测试覆盖守卫与技术文档，不修改客户端业务代码。

## 权威、维护与执行入口

`docs/product/reading-window-session-acceptance.csv` 是 RW 验收身份与预期结果的唯一来源；[覆盖登记](reading-window-test-coverage.json)逐条引用它，不能另设一份固定的 34 项 ID 清单作为权威。新增 CSV 行而未增加对应映射应失败，原产品 CSV 的 `not_run` 不被设计检查改写。

| 内容 | 维护位置 | 主责 |
| --- | --- | --- |
| 产品条款、验收身份及预期 | `docs/product/reading-window-session-{prd.md,acceptance.csv}` | AP／UI／PL／JA，与产品评审同步 |
| 逐 RW 的页面、状态组合、M／C、Owner、适用层与必需证据 | `docs/development/reading-window-test-coverage.json` | QA，与对应模块 Owner 共同审阅 |
| 真实设计交互断言与封闭能力常量 | `scripts/report/verify_interaction_gap_design.cjs` | UI／QA |
| 覆盖、执行、独立回执与图片完整性校验 | `scripts/quality/interaction_coverage.py` | QA |
| 临时目录中的登记／回执攻击及真实 Node 进程守卫自测 | `scripts/quality/test_interaction_coverage.py` | QA；这是守卫自测，不是产品通过 |
| 每次命令、stdout／stderr／log、source manifest、设计回执和截图 | `.build/quality/interaction-design/<unique-run>/` | 运行生成；后续发布按受控制品归档政策迁移 |

`validate_registry(root)` 验证登记；`run_design_coverage(root, out)` 实际执行唯一代码注册的 Node 命令并独立检查回执；`release_coverage_gaps(root, summary)` 逐条返回仍缺的适用层。登记文件不能指定任意 shell 命令或新增运行 adapter。`validate_design_receipt` 也供门禁／负例独立检查，不只相信汇总 `passed`。

门禁统一入口沿用 `npm run verify -- --change <id>`，其中本轮登记检查、守卫测试和设计执行分别接入 `interaction-coverage-contract`、`interaction-coverage-tests`、`interaction-design-execution`。完整发布还必须使用真实发布 profile；开发层设计执行成功不能吞掉缺失的产品层。

## 六类证明分别判断

| 层 | 必须证明什么 | 不能替代它的材料 | 本轮运行能力 |
| --- | --- | --- | --- |
| `design` | 独立 HTML 设计中的入口、状态、键盘／指针、几何、内容和故障态 | 纯检查名称、截图数量、静态设置值 | 已有唯一代码注册 Node／Playwright adapter；每次实际执行另存回执 |
| `product_component` | 新设计下的真实 React 组件，动作及可观察结果，公开 ApplicationPort 边界 | HTML 原型、仅构造 DTO、旧 57 项标题相似 | RW 专项 adapter 尚未实施，登记 `not_run` |
| `real_pipeline` | 实际事件入口→真实 Repository 持久化→全量查询／分页／恢复与序列核对 | 手写事件数组、设计内分页、只查总数字段 | RW-17～23／33 必需；专项 adapter 尚未实施，登记 `not_run` |
| `real_language` | 同包实际 worker／随包资源，token span／reading、Unicode 重构、当前句偏好和迟到隔离 | 人工样例 token、直接 publish 读音、ruby DOM 存在 | RW-26～30／33 必需；专项 adapter 尚未实施，登记 `not_run` |
| `native` | 最终包／资源身份与 OS 真实动作结果、原生 bounds／窗口、权限／快捷键／材质／多屏 | 浏览器拖动、CSS 毛玻璃、配置保存、旧包录像 | 专项 adapter 尚未实施；所需实机条件缺失时具体运行登记 `blocked` |
| `game` | 同一最终包的真实 CrossOver 游戏、Core 版本／资源、回调身份与画面对照 | 设计回放、旧包四游戏回执、截图单独存在 | 专项 adapter 尚未实施；四游戏按最终包逐款运行，Ever17不纳入 |

每个 RW 必须显式列出六层的适用性和原因。代码中的适用性政策不接受从必需改成“不适用”；真实层的 adapter 注册集合当前为空，登记里填一个看似真实的命令名或把 design adapter 改名为 native 会失败。RW-06／12本身要求实机证明，原型说明不作为该条设计层执行通过。其他条款可在设计层验证其表现，但仍保留相应真实层。

当前未实现 adapter 是能力缺口，而不是测试通过；无论开发门禁完成多少项，发布都会因必需真实层 `not_run`／`blocked` 拒绝放行。新增真实 adapter 需要审阅命令、输入／资源／最终包闭包、实际执行、独立结果解析和负例，不能只往 JSON 填 ID。

## 防止再次漏掉“未拍、未执行、被替身绕过”

设计能力由 Node 源中的实际 `await check(...)` 与 `RW_CAPABILITIES` JSON 常量交叉闭合。覆盖登记必须与这些代码调用逐项一致；未知 check、重复、漏映射、未声明的 RW、所有者或模块／组件不存在均失败。scenario 与 anchor 必须来自真实设计源的分支和 ID（含有限字面量的动态会话 ID），不能填写一个任意 URL。M／C 除存在性外还校验专项技术职责的最小集合：预览必须包含 C35，会话完整记录必须包含 C36，改填一组存在但无关的凭据模块／组件不能通过。

`design_states` 是供评审的计划标签，不是运行证明；程序不宣称自动理解任意自然语言标签。每 RW 的 `design_dimensions` 才是可执行覆盖，必须严格等于代码中实际 check 对该 RW 声明的维度，且回执必须出现对应观察。未进入可执行维度的语义仍需独立审阅，不能因标签非空宣称已覆盖。

除逐条映射外，代码固定以下最小行为维度，登记与执行回执都必须包含，不能删条目后继续通过：

- 分词、注音、词性三个独立开关的全部八种组合 `lexical-000`～`lexical-111`，同句实际观察 ruby／token／POS／原文重构；“开关已勾选”不足。
- Select 全部产品入口矩阵；鼠标／键盘唯一高亮与不重叠间距；长菜单内部滚动；空／加载／失败与键盘状态。
- 暂停跟随时的实际可见序列保持、多页全集序列核对；只比较行数会遗漏“最后一页被替换”。
- 预览取消保留对象与切换后旧帧隔离；页面演示仍明确不是异步系统帧验证。
- HUD `560px`／`500px` 两种宽度的直接动作可达性、固定栏与无重复菜单。

每个声明维度还必须有对应实际 `kind=observation`／`dimension`／`value`；单在常量里列维度不算执行。每个 check 的正整数 `assertion_count` 必须与成功 `kind=assertion` 记录数一致；零断言、布尔值充数、999自报、只有空截图记录、只有顶层 passed 都失败。断言记录来自 Node 对真实 assert 调用的计数；消费者只接受本轮实际使用的 `ok/equal/notEqual/deepEqual/match`，并复用 Node 原生 `assert/strict` 批量重放记录中的操作数，不另写一套 Python 比较语义。操作数矛盾、未知方法或缺参数直接失败；但“断言是否足以证明产品需求”仍需独立审阅。

## 本次执行身份与证据协议

Python runner 在仓库范围内创建不可复用的唯一运行目录，生成随机 nonce，并复用既有 `gate_core.source_files`／SHA-256 源闭包规则。闭包包含 PRD CSV、覆盖登记、设计源、QA 与消费者源码，排除 `.build`／构建制品等生成物。Node 按 Python 提供的完整文件列表重新逐文件读取并计算运行前／后的 hash；不能只抄环境变量中的树摘要。

schema 2 回执必须具备：

- 本次 `execution_nonce`、有时区的实际开始／结束时间、运行前／后完整文件 hash map 与树摘要；QA 脚本和覆盖登记 hash。
- 封闭 check 全集：真实状态、需求映射、断言计数及记录、维度观察、截图引用；不得通过筛选几个 check 就宣称 full-suite passed。
- 截图逐文件相对路径／SHA-256／实际 check 双向关联；必须存在，复用既有 `pngjs` 实际解码并核对尺寸，单图限制 32MiB／8192边长／16MP；不能只有PNG签名、不能逃逸目录／通过 symlink／冒用其他检查截图。
- 固定命令、实际进程开始／结束／退出码、stdout／stderr／log及各文件 hash；非零退出、缺失日志、被覆盖日志失败。Node 在写完回执后把 `verification_sha256` 输出到真实 stdout，Python核对该值和输出目录；事后修改回执再更新它自己的汇总 hash 也会因原 stdout 绑定不符而失败。
- `design_only=true`，product／native／game／global hotkey／tokenizer／live capture 均明确未执行。

Node 必须只在本次唯一输出目录产生一份新的 `verification.json`。Python不相信stdout中一个“passed”或任意旧路径，而是查本次目录、校验源码稳定、随机数、时限、全量 check、断言、维度、图片和日志。`release_coverage_gaps` 再核对真实 receipt／manifest文件和 hash，手填 `validated_by`／`status=passed` 的 dict 不能消掉设计或真实层缺口。

旧包、旧源、旧 nonce、旧成功或批准图不被升级；新候选输出独立保存，未修改原图、像素阈值、材质 token 或原产品运行状态。直接执行 Node 且没有完整 manifest 的临时预览只属于 `prototype_closure`，正式设计执行消费者不予接受。

## 真实流水线专项负例与后续实施顺序

代码只读检查提示了必须覆盖的行为：如果暂停分支早于 raw 持久化、只存最后样本、事件只进入 100 条内存诊断，或 GC 删除活动会话前段，即使设计能展示分页也不满足 RW-17～23。本轮将它们作为 `real_pipeline` 的独立必需证据，未改产品实现也未宣布修复。

后续按以下顺序接入实际运行：

1. **实际 Repository／事件流水线（AP／HK）**：选流前、未选流、重复／增量、暂停期间仍收到的 raw、Hook各阶段事件、超过显示缓存和多页；对已收到且确认持久化的序列做无漏无重核对。活动会话 retention／GC 不静默裁剪；磁盘满、背压、崩溃／重启明确缺口，搜索和导出与全量集合一致。
2. **真实词法（JA／UI）**：断网随包资源、同句八组合、Unicode／无reading／非法span／worker退出和迟到身份；不能手写 token喂HUD。
3. **真实产品组件（UI／QA）**：共享Select逐入口和完整状态、HUD工具栏／局部滚动／菜单，按新批准设计截图，动作后查实际结果，不只有状态变量。
4. **最终包原生与四游戏（PL／HK／QA）**：冻结同包 hash、Core／worker资源后验证窗口帧、权限、系统按键、原生移动／多屏、材质与回退、持续回调和当前会话完整性；受阻保留证据，不借组件绿灯越过。

产品测试源码继续按模块邻近维护：Rust领域／真实I/O在既定 crates tests，组件与视觉在 `apps/desktop/tests/`，原生／四游戏在 `tests/integration/{native,games,desktop}/`。尚不存在的 adapter只在此登记计划，不创建空脚本、不填假运行结果。跨层证据必须共享实际源／最终包／资源身份，而不是仅标题相同。

## 守卫验收与限制

本轮守卫自测在临时根复制实际权威 CSV，调用真实 validator，并有实际 Node 子进程 roundtrip；测试分别篡改登记和回执验证拒绝。主要攻击是：漏 RW／漏层／跨层代用、未知或重复 check、删除必需组合、缺 Owner／M／C、错 nonce／旧源／源漂移、旧时间／无时区、零或虚构断言、缺实际维度观察、缺图／改图／8字节伪PNG／目录逃逸／symlink、无关截图关联、矛盾断言操作数／未知断言方法、回执改写与实际 stdout 不符、缺日志／错命令／非零退出、手填通过摘要。测试命令是 `python3 -m unittest discover -s scripts/quality -p test_interaction_coverage.py -v`，实际数量及输出以当次机器回执为准。

这套检查证明登记与执行证据满足已登记规则，不自动发现所有未写进 PRD 或断言的业务错误。因此仍需独立审阅“操作是否正确、结果是否充分、是否绕过真实边界”，并逐次把真实缺陷转成会失败的负例。设计层、守卫层通过不可表述为完整首版产品通过。
