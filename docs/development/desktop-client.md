# 当前客户端运行与验证

当前完整设计组合（guided与R79历史页）已由用户明确审核通过，身份见[源版批准记录](design-approval-complete-20261031.json)；其他页面按[全页与状态闭包](all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

## 实现范围与目标差距

现行完整冻结身份以[基线记录](iteration-baseline.json)为准；技术入口为[研发基线](../architecture/first-product-engineering-baseline.md)、[阅读/会话技术设计](../architecture/reading-window-session-technical-design.md)及[词法/模型技术设计](../architecture/japanese-study-and-llm.md)。[客户端对齐总计划](current-client-alignment-plan.md)定义剩余工作，默认[guided设计](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)，R79历史页覆盖历史入口，完整61源已获用户审核确认。本页不授予设计、字段或发布批准。

| 入口 | 已有实现 | 尚未对齐的目标 |
|---|---|---|
| Core与接入 | 独立Core锁、ABI1七回调、Windows双broker、完整目标身份和关联接入 | 新包须校验真实源码/资源/签名；平台与游戏通过不能相互继承 |
| 历史页 | HistoryScreen已有两查询、独立cursor/generation、详情/修正/学习 | CP-11迁移R79布局、轻文字会话加载、旧范围失败提示、编辑全导航与focus/caret保留；固定水位/定位完整多页仍须真实验证 |
| 当前会话 | 独立首屏当前卡、真实SourceText/偏好/固定词卡、共享浅层服务选择器及正文/raw/Hook后置折叠已有源码；C1双列/窄窗与紧凑服务行正在逐项修正 | 用户运行包审阅已发现布局差异；固定实时组件入口用于持续确认，完整独立视觉/Native/游戏仍单独验收；摘要不能从已加载页推断全量 |
| 数据全集 | Repository固定水位/不透明游标，独立正文、全部提取与Hook事件 | 新布局下多页、重开、空间失败、保留与游戏事件归属须真实验收；显示50条不限制全集 |
| 配置与翻译 | 同源13协议/21预设/4离线目录、引导字段、版本化保存/选用、typed HTTP与Google授权基础已有源码及限定验证 | 模型发现客户端入口、离线真实资格/执行与完整页面/真实服务验收仍未闭合；目录存在不授予可用性，限定回归不关闭整页 |
| 平台与阅读 | 主窗/HUD/设置/学习、共享Select、真实词法/辞书、独立collector监督和只读消费者 | 当前组合的原生效果、权限/旧帧/休眠恢复、真实服务及同包四游戏须独立运行，不能由局部截图继承 |

源码存在、源码语义对齐和运行验收分别记录。既有存储、组件、provider fixture和有限原生adapter按登记范围复用；未执行写 `not_run`，真实开跑后外部条件不足写 `blocked`，已有失败保留。原111条客户端用例、128定义/153组合、RW适用层及当前70来源/576观察的验收义务均保留。

## 使用当前客户端

内部pending文件锁沿已有进程拥有的窄guard管理，不增加游戏启动产品能力。当前源码部分实现与设计目标/真实运行分别按整页计划核对，不从某个缺陷已修推导整页完成。

首版运行环境为macOS arm64＋CrossOver，连接用户已运行的Windows游戏；客户端不自动启动游戏、接管Steam或结束整个Bottle。具体已安装版本、签名与资源身份须读取该包实际 `build-manifest.json`，不从仓库目录名或本页推断。

1. 启动目标游戏，在兼容环境中核对真实Wine路径与Bottle；从实际窗口列表选择，或通过受控点选取得目标。
2. 在同一核对面板检查环境、完整EXE路径、Windows PID/创建身份、架构及窗口；选择已有库、新资料或仅本次归属，确认“关联并接入”。预览/选择本身不注入，确认后才接入。
3. 收到候选后显式选择正文流。原文先显示，译文、词法与按需学习各自呈现状态和来源；无正文不把“已关联”或“宿主已连接”显示为取文成功。
4. 按需使用HUD、暂停/继续、重译、复制或固定句学习。主窗显隐、暂停、自动翻译与断开独立；关闭主窗不等于退出应用。
5. 查看正文、全部提取或Hook记录时，搜索/导出按声明的全会话或固定筛选范围处理；暂停不会删除已接收raw，不从最近缓存推断完整性。
6. “解除关联”仅清窗口关系并保留取文；“断开”结束本次自有接入。退出须完成producer、FIFO writer及原生资源收尾，不结束游戏或其他实例。

同一完整TargetIdentity复用会话、已选流和历史，不重复注入；不同目标先明确确认切换，取消保留原会话。失败保留有效关联并显示实际阶段，重试先fresh核验；同名EXE、裸PID或Bottle显示名不能判定同目标。Hook和预览权限/画面可用性独立；拒绝录屏不阻断取文，旧帧保留原时间，不冒充实时恢复。

历史页目标已同步到[阅读技术合同第15节](../architecture/reading-window-session-technical-design.md#15-历史页会话目录记录分页与固定阅读对象)及CP-11：目录/记录各自固定scope/cursor/loaded IDs与request epoch，replace/append失败保持原语义；换档/过期结果隔离，编辑禁用全部导航并保留草稿焦点/caret，显式定位复位范围且后续全量无漏重，词卡/重译/精读固定unit/revision。R79轻文字操作的hover/focus/disabled/spinner/error/窄窗/200%逐项验。当前源候选已部分实施CP-01当前卡和后置折叠结构；是否已安装该候选、其签名/源码封印及真实运行结果须查本轮工件与回执，不能从源码或这份说明推定用户正在运行的新包。其余差距与完整业务验收仍按表中边界保留。

## UI修改期间实时确认

从仓库根运行 `node scripts/dev/live-ui-preview.mjs`，使用固定入口 [http://127.0.0.1:1421/](http://127.0.0.1:1421/)。这是独立开发入口，直接导入生产 `WindowRoot`、窗口/业务组件和样式；保存 TSX/CSS 后由 Vite HMR 更新，不需要每次编译、签名或替换应用。启动器提供状态和停止命令，重复启动不结束占用端口的其他程序，入口说明见[实时预览使用说明](../../apps/desktop/tests/live-preview/README.md)。

外层工具栏不计入客户端视口。Main/历史使用实际1040×720 iframe，设置为1000×720，窄窗为760×560，根原点为0；不能在大浏览器里只给内部div设宽度并据此判断媒体查询。日常修改先在此入口核对页面、主题、连接/断开、长文和滚动位置；“设计对照样例”提供同源稿标题/台词/译文，仍明确为内存样例和未测试服务，不模拟采集成功。Main角色字号和重复信息已按阅读合同收敛；独立截图候选与失败原件另存，批准源和阈值不变。

预览明确使用受控内存样例，不读取个人资料库或执行模型、凭据、Hook和系统窗口动作。尚未安装真实Rust表单fixture的配置流程保留未就绪状态；不能据此宣称全部服务已可配置。浏览器可以确认排布和组件交互，原生毛玻璃、权限、焦点、持久化和游戏仍由准确源码的签名包另验。当前已安装包与正在热更新的源码分别记录，不能将实时预览当作包已替换。

历史与停止会话以[三页批准](design-approval-layout-refinements-20261107.json)为目标；通用设置以[结构稿批准](design-approval-general-settings-20261107.json)及[唯一入口](../design/first-product-v1/candidates/general-settings-structure-20261107/index.html)为准。当前通用页收尾见`.build/general-visual-closure-20261108/`：侧栏说明与导航分离并自然贴底，导航采用批准的37px行高/3px间距/12px字；General显式使用单层shell-fill的局部WindowFrame变体，默认Main/HUD/Learning及其他设置框架不改。General窄窗按批准viewport950/620处理，不以缩放后的内容宽度触发另一套整页底栏。真实scope/字段继承/CAS、Session导航和系统端口权威保持。原24像素红结果、200%布局红图、旧包和旧实机记录保留；同源reload也能复现24像素，不能把一次迁移截图绿色当稳定。读取中/读取失败/保存失败源态和Native参照环境已分别获用户确认，精确75原件及批准记录固定；真实组件和同包执行仍须验收：General固定14状态/27capture，独立源、生产React与签名包私有组合根分别取原始整窗图，0RGBA且无裁切/遮罩/缩放。 Native同时要求同primary显示器/原ICC、完整窗口包含和三阶段环境绑定，跨显示器差异不靠UI补色；严格27项批次锁拒绝失败续挑。Playwright继续验证真实组件交互，公开WK源观察器的资格不能冒充同包Native；受控Native也不证明OS登录项、真实数据库或游戏。默认C04仍复用，General只增有限呈现变体以对齐批准菜单，不复制选择业务。最终source、安装包、PID和验收范围由本轮实际回执分别绑定，完整产品门禁仍须全部满足。

## 构建与包身份

构建入口为 `python3 scripts/build_desktop.py`，需要仓库锁定的Rust/Node/npm、macOS构建工具、LLVM-MinGW和已准备的Core/语言资源。精确版本、资源hash与许可按技术锁和清单核对；构建时不自动升级技术栈。

- 默认输出为 `dist/Luna.app`、对应版本ZIP及 `build-manifest.json`；版本从构建输入读取，不沿用旧版本文件名。
- 候选使用 `--output-root` 指向 `.build/` 下新的专属目录；脚本拒绝复用非空候选目录，保留旧工件与失败证据。
- `--skip-windows-build`只复用已准备的Windows宿主，`--no-install`只复用现有npm依赖；使用前必须核对它们与本次锁定输入一致，不能把跳过构建当验证完成。
- 签名可用 `--signing-identity` 或 `APPLE_SIGNING_IDENTITY`选择实际有效身份；仅需要ad-hoc开发候选时显式传 `--ad-hoc`。签名模式、证书与designated requirement从真实包检查，签名不等于公证或发布批准。

包内源码封印、实际EXE、Core/语言资源、许可、签名与ZIP须独立核对；外部manifest不能重标旧签名包。生成成功只得到开发候选，不能据此宣称预览、词法、服务、四游戏或完整发布已通过。

## 代码与进程边界

```text
Windows游戏 → 独立Core/Host/Injector → 同位数Rust broker（JSONL v2）
                                      → hook-client → app-core → app-infra SQLite
React各窗口 ← 窄Tauri / ApplicationPort ← Rust权威状态
Rust应用 → 独立language-worker → MeCab/UniDic/包内辞书
Rust平台协调器 → 独立capture collector → 授权单窗口最新帧
Rust ModelGateway → 13协议HTTP wire（来源/资格与完整客户端各自验证）
                 → 离线候选待真实资源/授权/lease与执行接通
```

Core独立仓库消费锁定产物，不运行完整Python/Qt客户端。UI不直连HTTP/SQL/FFI或系统凭据，不用设计fixture构造产品台词、译文、词典或帧。M10维护三数据集和迁移，M11维护偏好，M12维护窗口/预览，M16维护秘密；collector、Hook、词法和拟定翻译worker各自有协议与生命周期，不互相承载业务。

## 验证与准出

开发执行 `npm run verify -- --change <id>`，CI另传Git base；任何实际源改动均须独立非作者review绑定完整当前source tree及actual changed files，缺失/同作者/陈旧/漏路径直接拒绝，metadata不能跳过；产品UI变更必须有注册的真实候选捕获与独立比较，最终包按实际工件运行适用原生、存储、语言、真实服务及四游戏场景。准出入口和要求见[工程门禁](engineering-gates.md)与[当前覆盖](current-product-test-coverage.md)。

所有运行结果另建记录，绑定源码、包、资源、环境、命令/nonce、日志及实际断言；原计划CSV保持定义和not_run状态。SR中文版只证明真实中文原文/直通/非日语分支，不能计作日语质量；MO、Never7和サクラノ詩10th须实际日文与质量证据，Ever17排除。守卫自测、设计原型、编译、HTTP成功或旧包结果均不能代替本次适用产品执行。

历史客户端切片、原文与实机证据：[只读归档](../archive/documentation-r75/docs/development/desktop-client.md)。
