# 编码前工作卡：库封面、绑定、预览与排障日志

> 2026-10-21后续修复卡RF-04已正式纳入；下方R30初始卡与历史结果保持原样，当前采用基线仍为R32，不能以本卡更新指纹。RF-04仅实现安全拒绝与可诊断恢复，不表示MO原生预览已支持。

## RF-04 / 现R32后的实机窗口标题证明缺陷

| 字段 | 本次精确定义 |
| --- | --- |
| 来源 / 现状 | MO非Unicode HWND被旧标题挑战写出nonce乱码；独立API实验42项+4负例，不证明通用替代API安全 |
| 需求 / 页面 / 状态 | FP-03/05/12、RW-01～06、REQ-052；connection-sheet与当前会话预览：unsupported/unavailable、恢复失败、Hook仍接入；不生成新批准像素 |
| 模块 / 组件 / Owner | M02/M12/M14/M15；C13/C14/C35/C36；Root分配生产与独审，repair_change_governance管本卡与维护producer |
| 生产允许路径 | crates/hook-client/src/window_binding.rs；crates/lunahook-host/examples/target_probe.rs（含同文件维护测试） |
| 治理 / 资源路径 | 本change、runtime-followup/plan.md、scripts/quality/product_execution.py与test_product_execution.py；双架构target_probe.exe及core-staging-receipt.json，详implementation-authorization |
| 实施行为 | 非Unicode HWND写前拒绝，现有类型明确unavailable；Unicode保持身份/超时/exact nonce与恢复检查；恢复错误显式保留；Hook独立 |
| 验证 / 完成 | 实际product-hook-client-tests、product-target-probe-tests执行且非零；A/W、日本语、身份/超时/restore失败维护负例；双架构资源重建与新包实机，Hook/窗口标题前后独立核验 |
| 排除 / 状态 | 不改公共DTO/DDL/依赖/窗口几何，不换A/W/位数重试，不自动修写MO标题，不刷新基线/批准图；implementation_in_progress，产品not_run |

RF-05另案登记：SR退出后UI断开但native active未释放导致MO受阻。session-scoped退出、有界异步drain及expected SID/target compare-clear、迟到诊断不污染新会话为待定位义务；精确路径待只读报告，未授权扩大生产面。

### RF-05正式生产卡（独立于RF-04）

- 产品/设计：FP-03/05/12、REQ-052、RW-01/02/04/17～23；同一connection-sheet与会话断开/清理中/可重试；不改批准像素或让UI绕过切换确认。
- 模块/组件：M02/M10/M12/M14/M15；C13/C14/C15/C35/C36；Owner natural_disconnect_fix，Root与独立Owner审阅。
- 精确生产路径：hook-client/src/lib.rs、app-infra/src/hook_backend.rs、app-core/src/application/runtime.rs、app-core/src/ports/runtime.rs、app-core/src/ports/mod.rs、app-infra/src/storage/database.rs；现有app-infra/tests/application_runtime.rs及内联测试。完整仓库路径见implementation-authorization与verification。
- 机制：session-scoped ProcessDisconnected→supervisor有界stop→producer/relay/EOF/FIFO drain→expected SID/target compare-clear active/preview/锁；清理中typed可重试。consumer不自等barrier，旧A诊断/状态/封句不能落latest B；活动他目标不被清掉。
- 验证：实际hook-client/core/infra/native-ipc维护producer；自然退出后重接、stop/drain失败、迟到A/活B、原互斥/FIFO保持；最终包SR自然退出后MO接入不需重启Luna。当前implementation_in_progress，产品not_run；不扩公共DTO/DDL/依赖/UI。

RF-04维护WinAPI附加：scripts/quality/window_title_proof_api.py、fixtures/window_title_fixture.c由repair_independent_review实现，product-window-title-proof-api-tests使用真正双架构Rust probe及自有offscreen非激活fixture，执行器独立核source/build/EXE/nonce/逐项断言；不是产品实机通过。

RF-04原allowed hook_backend.rs窄错误投影由natural_disconnect_fix负责：受控requires_unicode前缀→window_title_proof_requires_unicode与中文安全message；其余generic原样，不透传raw或扩DTO。实际WinAPI producer固定30项（A/W架构矩阵、身份拒绝、截断/非法UTF16、恢复/EOF/第三方/超时），7源及4个probe/fixture实际PE/hash/日志、nonce与断言均独立核验。

RF-04追加决策：原实际WS_VISIBLE fixture抢焦点前置失败保留；现strict hidden visible0/foreground0且安全失败立即终止。为修复真实窗口挑战后隐藏/最小化的恢复漏洞，proof按强bound_identity独立于discovery visible/title过滤，原枚举/native唯一nonce不变。两源内部helper CLI扩为HWND PID IMAGE CREATION NONCE ARCH CLASS，全生命周期核根HWND/PID/image/FILETIME/冻结arch/class+strictcaption；不扩wire，hidden不等预览支持。执行器从原始fixture/probe字节与process.json独立复算并拒绝case自述/raw矛盾。

| 字段 | 本轮定义 |
|---|---|
| iteration_id / change_id | repair-library-binding-preview-diagnostics |
| 当前基线 | FP1-20261021-R30；737项；SHA a6cd5a0f387ec9a9e2e235c5e145375e2b60317425742b2709b2012c3ca2ff4d；保持不变，不自动升级 |
| 源码前身份 | 无 Git；source_files 1609项；ee266096a78a703c2c07f8ff48d66c2c6a399da131e266ba3878e624472a0d81；与09包记录及R30冻结完整map逐项一致 |
| 变更前镜像 | .build/hook-lexical-retest-20261021-010406/change-plan/source-before.json；before-source/21份真实字节；before-source-index.json |
| 用户授权 | 本轮反馈要求修复五项；封面“自动读取本机封面，无图时自动保存首张游戏画面”、启动“客户端只连接已经运行的游戏”均resolved_user_explicit；并保留此前按架构开发与对应回归授权 |
| 本工作项实际写范围 | 仅本change及.build/change-plan；源码/现行PRD/设计/架构/guard/baseline/批准PNG不由本Owner修改 |
| 主责 / 独审 | 计划Owner /root/repair_change_governance；产品实施由Root分配AP/UI/PL/HK；独审角色由Root指定，不能由实现者自签全部通过 |
| 当前状态 | 计划可审；产品精确源码路径、资产端口/诊断DTO、设计状态及authority同步尚待Root；不标ready或已实现 |
| B门禁 | B0规范/设计/合同映射先补齐；B1真实Core取文，B2资产/会话持久，B4离线日语词法，B5同包四游戏；工程静态不替代实机 |

## 封面工作项（既有 REQ-038；FP-02）

- 关联：REQ-038/REQ-043/FP-02；GL-06/09/10及RW-01/03/05/06相关。原条款支持元数据封面；新单张封面例外是用户本轮覆盖，不假称GL原文已验。
- 设计：index.html#main-content，库首屏、C11卡片/详情、C12元数据；正常本机封面、无封面、不可读/损坏、正式关联首帧保存中/失败、取消候选不保存、重启恢复。新状态必须单独候选，不覆盖原PNG。
- 模块/组件：M01/M10/M12/M13；C11/C12/C35。AP管游戏引用、infra管安全资源；PL提供已核验目标帧；UI只走端口。
- 复用：既有games.cover_asset_ref、游戏保存与查询。没有证据显示安全托管资产端口已实现；先出精确协议/路径及编译校验，不新增未经授权表/字段/依赖。
- 可观察：本机Steam/游戏真实资源显示；无图且正式关联、身份有效、权限允许时仅首张有效帧存托管资产；无权限/取消/旧generation不写；不覆盖旧封面。重启仍显示实际文件而非设计图。
- 验证：真实文件/安全资源读取与写入负例、DB引用/回滚、真实组件有图截图、正式包本机图/首帧/重启；只查字段或无图fixture不算通过。
- 完成：资产字节hash、来源、game/target/generation归属、包身份、实际截图可核，失败和未执行明确。移出条目不删除游戏文件；资产回收仅按经审既有策略。

## 单面板绑定工作项（既有 FP-03；GL-11～20）

- 产品：REQ-002/004/043，FP-03；GL-11/12/13/14/15/17/18/19/20。
- 设计：index.html#connection-sheet；C13列表/点选、C14核对同屏、C15接入进度/候选。未选、映射不唯一、自身拒绝、同目标复用、不同目标切换/取消、接入失败、已接入等待文本、候选/正文选择。
- 模块：M02/M12/M13/M14；C05/C13/C14/C15。复用application-port、真实身份/窗口映射/HookBackend，UI不直接FFI。
- 可观察：从全局入口连续操作，只有一次关联确认；该确认立即触发Hook，无第二“核对目标”确认。选择本身不注入。记录真实启动/回调，而非按钮被点当成功。
- 验证：真实入口点击链+注入调用计数、完整目标身份/PID复用拒绝、取消保留旧会话、正文候选显式选择；正式包实际Core日志/回调/游戏句独立核对。

## 只连接已运行游戏工作项（既有 FP-02；REQ-037）

- 本轮用户明确覆盖旧“运行并连接”首版行为；旧原始PRD保留。首版全部游戏启动用户入口移除，详情三个重复入口删除；页头全局连接仍可发现；会话必要更换/解除/断开不误删。
- 设计：index.html#main-content；C09/C11/C13；空库、未选/已选、运行/未运行、已连/未连、卡片单击/双击/键盘/菜单。
- 模块：M01/M02/M13，UI主责AP复核。卡片不新增自动启动/注入手势；broker/helper正常管理保留。不是扩成删除全部底层启动实现的授权。
- 验证：完整用户入口与快捷操作扫描及行为断言；已运行目标可接入；未运行不误报成功；旧启动UI测试必须有合法需求变更记录与独审，保留旧断言历史，不能静默删测试换绿。

## 自动常驻预览工作项（既有 RW-01～06）

- 产品：FP-03/05/12；RW-01～06；GL-11/12/15/18/19。新要求覆盖旧RW-03每次手动开启，不绕过OS权限。
- 设计：index.html#connection-sheet、#main-content；C35/C34。无目标、starting/live、缺/撤权、授权恢复、最小化/不可采集、退出、候选切换/取消、关联后跨页签。
- 模块：M02/M12/M13，PL权威lease/generation/权限，session owner独立于页签。
- 验证：权限已具备时不点开启即连续两次真实变化帧；绑定后正文/全部提取/日志跨页签保持；撤权/退出/迟到帧不能伪实时或写错封面；Hook和离线词法不依赖录屏。记录窗口尺寸/焦点未变。

## 有效排障日志工作项（既有 REQ-052；FP-12）

- 产品：FP-09/12、REQ-052、RW-17～23。当前Core已输出有用事实，损失在产品投影；不回退完整原客户端，不改七回调ABI。
- 设计：index.html#main-content；C36/C33。接入阶段、实际x86/x64 DLL、native Console/Warning/Info详情、具体失败/退出码、默认降噪/全部、搜索/分页/导出/缺口。
- 模块：M02/M10/M14/M15/M13；HookUpdate→HookEventInput→repository→sessionRecords；typed native facts与有界脱敏文本合同待精审。metadata_json已有但不能塞无约束payload；不新增schema。
- 可观察：显示引擎/扫描范围/Hook地址/原生错误与资源真实信息；Warning1/2正确；未知kind保留。helper请求成功不等握手，更不等正文成功。默认text_unselected降噪，但全部提取与warning/error/缺口仍完整。
- 验证：同一实际SID的stdout/native事实→adapter→DB→DTO→UI逐字段核对；1000未选事件夹注入错误、重复warning计数、50+多页/固定水位/搜索/导出；凭据/路径/NUL/控制符/超长Unicode与可能全文负例，不删除既有安全测试目的。

## 共用验证、依赖与完成条件

当前verification只登记真实已注册check ID，不虚构adapter；本次仅跑validate_change与OpenSpec strict。现有product-core-tests/product-infra-tests/product-native-ipc-tests/product-reading-component-tests/编译等需补适用断言后真实执行。新的资产端口与五项视觉状态未接线，不能把登记当覆盖。product_visual_checks/product_runtime_checks空，发布继续缺项拒绝。

前置：已核对的本机封面或受控无图目标、真实可推进四游戏、同一签名新包、可见解锁桌面、实际录屏授权、既有Core/语言资源。日文词法必须用真实日文句与worker/span/ruby，中文SR不替代。模型服务不作为五项修复或离线词法的前置。

09旧证据：run.json仍not_run；SR会话1792516708809-21018-0曾有21源/147原始文本/11单元/16修订，但0日文/0词法快照；UI未独立验。只证明局部Hook，不是本轮五项或四游戏全通过。原61工程检查仅旧09身份。

下一步先独审并由Root精确同步当前PRD/整稿/技术设计/验收与合法旧测试变更，再确定允许源码清单和新基线审阅；不把本change的计划通过当整稿获批准或产品已通过。完整实施后新包同身份依次SR、MO、Never7、サクラノ詩10th，Ever17排除；每游戏单独记录passed/failed/blocked/not_run。禁止改游戏宽度/DPI/Bottle后端、影响另一台Steam会话或循环重启Luna。
