# 实施设计与边界

遵循docs/architecture/reading-window-session-technical-design.md与R20的目标契约，用户已确认设计绑定design-approval-20261019.json的16源/37图。编码先写具体契约冻结记录；实现不能修改原稿迁就代码。

- C04沿已安装React Aria、自有CSS；不新增前端框架。C16复用真实tokens保持八组合、错误保留原文。C18常用入口前置，更多去重，正文/菜单局部滚动。
- M12原生窗口协调、独立受限预览帧通道、真实按下边沿快捷键、scope有效态与补偿、原子平台geometry。可信窗口实例映射；无法核验时返回no_capture_window，不猜标题/截全屏。
- M03/M10 raw先持久再消费；有界回调不写盘；三集合typed query/高水位游标/搜索/导出/会话保护。001不原位修改；002字段/关系、独立schema、迁移备份和拒绝旧包路线先独审。
- DTO权威Rust生成TS；UI只ApplicationPort；root负责bridge/IPC/构建/门禁，UI、平台、数据三Owner独占文件，改交界先协调。
- 当前source audit/catalog在构建前按实际源码重新独审；新candidate不覆旧dist。运行包身份/实测结果只写独立.build记录，不build后回填已封装source。
- 旧acceptance-v3/批准图片保留，最新批准源的同态候选参考独立输出/审阅；actual React/原生/语言/数据/四游戏分别留证，零视觉阈值不放宽。

## 候选02实机串句修复

SR新会话已接入但首句正文尚未到达时，可见HUD显示了上一款游戏的句子；候选02保持失败，不继续跨游戏验收。当前阅读对象由Rust快照权威选择：opening/active Hook存在时只取该会话最新可见正文，没有则为None；暂停保留同会话末句。没有运行中的Hook时保留原有全局末句恢复和新手动输入行为。手动页在Hook运行期间按自身receipt展示，不抢占游戏HUD。历史完整保存，显式固定旧句精读不被当前会话清空。

查询须按会话在数据库中选择，不能过滤已截断的全局50条history充当完整查询。新增真实SQLite/runtime和RBC-29～32覆盖首句前等待、旧会话迟到、新句动作身份及断开后同会话末句；保留原28组件检查、79行为与37状态77变体。修复后独审、重封存、重新打包，并从SR开始执行同包四游戏回归。

正常退出追加实机修复：候选02的AppleEvent Quit绕过Tao的ExitRequested而留下active会话；以现有AppKit delegate的applicationShouldTerminate延后答复，所有Quit复用既有异步shutdown/drain/FIFO/Disconnect。已有selector拒绝安装，不swizzle或换依赖；失败reply NO并可重试，成功才批准系统退出。源实现、Cocoa受控测试、正式包两种Quit的末尾持久事实与broker清理分别验收。

系统答复通过独立的main CFRunLoop common-mode块异步执行；Tauri Task返回释放Tao回调锁后才能reply YES，NO与Rust重试恢复在同一块内完成。禁止用主GCD串行队列重入或持框架锁同步reply掩盖退出死锁；原Cocoa03仅覆盖AppKit，不外推为Tauri链路通过。

## 候选04实机历史刷新修复

R25工作项见 `r25-runtime-closure-work-item.md`。固定高水位限制正文集合、原文版本与顺序；同一范围内已持久化的译文和人工修正仍可更新。UI轮询和重试须读取所有已加载页的不透明游标，并核对同generation、会话/数据集/快照和稳定行身份后原子合并；范围外新正文只更新新到数量。不得用当前句结果贴到任意历史行，或以首50条覆盖已加载旧页。失败保留最后成功内容，迟到旧条件响应拒绝，暂停跟随时保持逻辑行锚点与像素偏移。Rust查询/SQL/DTO与已固定精读对象不变。

截图前提的 `hudFitHeight` 必须来自同一次实际渲染的 `hudFit.clampedHeight`；原断言、独立消费者、参考图及阈值均保留。缺字段的旧回执保持失败，新结果只来自修复后重新执行。

## 候选05实际预览图片的 CSP 修复

R26工作项见 `r26-preview-csp-work-item.md`。系统授权与WebView图像呈现分别验证：05旧签名授权已恢复为当前签名，仍实际显示破图；真实React组件使用的内存JPEG Blob URL被正式 `img-src` 拦截。产品只为现有图像通道补齐 `blob:`，不改变 script/connect/default 规则、技术栈、DTO、原生采集或存储。

组件和全部候选页面须在首次导航时加载同一正式CSP并核对实际响应头；含帧截图必须真实解码JPEG、核对自然尺寸。原CSP的受控失败、新CSP同组件同图片的成功及37状态77变体零差异分别保留。原批准图、阈值和原38组件断言不改。native Live/时间戳或img节点不能代表用户已看到窗口内容，最终新包须核对真实游戏和客户端的变化画面。

## R27：有界未完成续写V2

工作项见`r27-capture-continuation-work-item.md`。新具名策略避免1500ms静默将未完成片段永久封口；仍以现有6000ms/UTF8/revisions/refs硬限和生命周期边界有界。完成判定须核对引号/括号配对。新processing revision/snapshot明确规则，旧V1与失败记录保留。原文可修订、finalized才自动翻译与精确版本结果不变；不改UI/Core/schema。无页信号的快速无标点换页和长于硬限的输出仍是限制，不能承诺任意GDI整页。

## R28：唯一未闭合引号的严格后缀增长

工作项见`r28-quoted-prefix-growth-work-item.md`；独立架构方案审阅为Medium、有条件允许在既有缺陷修复范围内实现，尚非源码或实机验收。V3只允许正确配对前缀之后唯一明确的未闭合引号后缀，且该完整后缀含非空白内容、是incoming的严格前缀时，生成保留prefix的局部增长快照；完整GrowingSnapshot优先。不得泛用最长后缀/前缀重叠、归一化、吞合法重复或游戏/人名特判。错配/多锚点/非增长等不适用时保持既有明确行为，不宣称可靠页信号。

Domain给出UTF8 prefix字节位置；单writer按旧ref_ordinal逐span消费保留字节，支持跨refs、非零raw起点和合法字符切口，再追加incoming实际span。新refs精确重构必须等于新原文，旧revision/raw/refs不改；缺来源、越界或重构失败须拒绝并进入现有有效的未确认完整性记录路径，不能伪造正文成功。按prefix_bytes+incoming字节和实际保留refs+1核原6秒/64KiB/128边界。V3 processing revision、policy/snapshot/cache身份与fold/保留prefix如实记录；旧版本open先按版本处理或封口，ingest/完成/timer统一政策，finalized+latest自动任务资格不变。

原07工程绿、SR08失败及R27 authority原字节保留。新源需独立代码/治理精确审阅，采用R28并重打候选08、完整verify及同包四游戏；不从07继承任何运行通过。

## R29：稳定开发签名与真实包身份

工作项见`r29-stable-developer-signing-work-item.md`。修复只在M15的三文件中完成，不改UI/Core/数据库/依赖/批准设计或系统TCC。`--signing-identity`与`--ad-hoc`互斥；CLI身份优先于非空APPLE_SIGNING_IDENTITY，再取唯一valid codesigning identity。显式身份不可用、没有或多个可选身份、最终签名失败均失败，不降级、不轮询试签别的身份；`-`不替代显式ad-hoc选择。解析到的同一leaf SHA1身份用于Tauri与资源seal写入后的最终签名，并保留既有entitlements/flags，但不保留旧ad-hoc requirements。

新包签名后读取真实mode、Identifier、TeamIdentifier、Authority链、CDHash、DR及leaf证书SHA1，核对开发证书选择；ZIP identity和outer manifest保存实值，消费者在解包后执行原codesign严格验证、源/资源seal/EXE/ZIP身份及实际签名metadata对照。不能只信env、预期名称或外部标签。新metadata的完整性不得通过删去部分字段退回旧分支；兼容判别应绑定签名seal内的最小合同marker，使删去两份外部新metadata也不能把新包降成旧包。当前合同marker为seal内`signature_contract`、`signing_mode`、`expected_leaf_sha1`，签名后`signature_metadata`同值保存于ZIP identity与outer；marker不含签名后CDHash，不制造seal循环。该约束防外部metadata删除/修改造成降级；不声称防攻击者有能力重签整个.app并改写seal的分发认证威胁，保持既有local build provenance边界，不引入公共API或全局信任名单。无新marker/新字段的旧schema1包仍保持原有严格验证，不补造开发身份。具体私有字段名由Owner在三文件内实现并独审。

参数/签名选择与消费者负例使用现有PackageIdentityTests及已注册package-identity-tests；原跨源、资源seal、无效签名、改包、ZIP路径等断言不可削弱。新包实际codesign与初次用户授权后画面、正常重启/后续同证书另一构建的权限情况分别留证。两个原生探针只证明签名能力及DR关系，不替代Luna/TCC/四游戏，也不自动提升发布状态。

## R30：生成物直接与间接身份一致

工作项见r30-generated-worker-closure-work-item.md。以已审生产生成的language-worker实际字节为唯一新值，只更新status.product_sources_sha256对应一key；其他447产品文件、模块/组件/能力/历史与实机轴完全不变。原render若输出一致则MD/CSV不写入候选目标。R29原733项按原字节归档，R30候选只变这一冻结status项并新增本卡/归档/精确审记录。四消费者必须含真实validate_catalog闭包检查，不用直接baseline通过代替间接pin一致。旧08包/partial失败保持，另建09并重新完整verify；无新UI/代码/框架、TCC或发布通过。
