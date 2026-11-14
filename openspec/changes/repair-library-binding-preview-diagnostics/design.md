# 编码前技术设计与复用边界

## RF07：固定历史范围的有界恢复

本增量对应[Root工作卡](rf07-history-range-recovery-work-item.md)，M10会话记录/M13 UI、C36 SessionRecordPanel；复用当前整稿main-content下正文历史/全部提取/Hook日志、既有跟随/暂停/回到最新控件。Rust端固定范围、1800秒TTL、GC、cursor身份、DB与公开DTO均保留；UI只使用既有application-port和typed错误，不解析文案来猜测过期。

- follow=true且当前请求返回typed snapshot_expired时，单次尝试cursor:null。新范围完整读取成功才替换可见旧行与cursor；新读取失败保留旧行、错误及显式重试，不递归reset或持续重试形成请求循环。
- 暂停/浏览旧行时保留原行、滚动锚点与冻结范围，过期只提示恢复入口，不自动跳底。invalid_cursor（含GC）和cursor_range_changed无论是否跟随都不自动当作可恢复过期，明确由用户“回到最新”启动cursor:null。
- 其它typed错误、一般失败不自动换范围。现有重试遵守错误种类与当时用户跟随状态；不能用清空数据或hide-all错误制造恢复成功。
- session、dataset、filter与请求epoch共同隔离响应；切换/卸载/取消后的旧请求及迟到恢复不得覆盖新对象。维持single-flight，自动恢复仍服从有效请求身份与用户最新暂停意图。
- 真正恢复当前查看范围不重注入、不切正文、不暂停游戏取文，不改变正文/译文/词法记录归属。旧已加载内容保留可复制，完整分页/搜索/导出义务不减。

验证使用真实React组件和现有受控端口的typed错误/延迟，保留全部原断言；组件回执不能替代新包实机恢复。candidate12四句取文成功和historyexpired失败按原身份保留，candidate13及R34均未由本段宣告通过。

## RF-04实机安全修复增量

R32/candidate11后的MO回归发现非Unicode HWND旧W标题挑战遗留nonce乱码。独立API实验只支持立即收敛：任何标题写入前拒绝非Unicode HWND，既有typed不可用原因继续投影；不换A/W/SetWindowTextW/位数重试。Unicode挑战仍以完整身份、exact nonce、有界等待及显式恢复回执判定。Hook进程取文与native预览证明独立，预览不可用不等于Hook失败；不能将未证native归属标已绑定。公共DTO/架构/DDL/依赖不变，非变异身份新方案留后续审阅。

两个真实维护producer直接运行hook-client库与target_probe example测试，拒绝零测试/跳过/过滤/失败/源漂移；跨进程API lab、guard夹具与正式同包游戏回归分别记结果。精确双架构probe资源由原构建receipt更新并重新freeze，Core与原批准像素不动。RF-05退出后active残留另列工作卡，不将其诊断为标题转换造成。

## 现状、来源与确认

现状差异和原版参照分别在 `product-gap-review/review.md` 与 `log-diagnosis/report.md`。原版可参考的是 HostInfo 类别、原生内容、时间和排障事实保真，不引回 Python/Qt/翻译业务。完整 Core 七回调与 broker JSONL 已保留诊断，不需要改 native ABI 或更换 hooks-core。

用户明确覆盖两条旧规则：首版只连接已运行游戏；允许无封面时在正式关联目标上自动保存首张有效画面作为静态封面。原 `RW-03` 的每次手动启用预览被本轮“预览常驻自动开启”要求覆盖；持续预览仍不录制、不持续保存、不上传、不含音频。单张封面例外必须限定归属与时机，不能扩成任意窗口截屏授权。当前 authority 由 Root 另行同步，旧批准图保留，新视觉候选独立输出。

## 模块与状态归属

| 子项 | 模块 / 组件 | Owner 与复用 |
|---|---|---|
| 封面来源、托管资产、归属 | M01/M10/M12/M13；C11/C12/C35 | AP 管游戏及封面引用；infra 只在已授权目录读本机资源；托管资产读写端口待精确设计；PL 提供核验目标首帧；UI 只收安全资源投影 |
| 单面板绑定 | M02/M12/M13/M14；C05/C13/C14/C15 | AP 保有完整运行身份、复用/切换和注入状态；UI 局部选择不注入；复用既有 application-port 与 target mapping |
| 只连接已运行目标 | M01/M02/M13；C09/C11/C13 | UI 去游戏启动入口及详情重复入口；AP 保留既有安全能力，必要 broker 生命周期不受影响；旧启动测试须合法需求变更审阅，不能静默删断言 |
| 常驻预览 | M02/M12/M13；C35/C34 | PL 保有目标 lease、generation、采集状态和 OS 权限；session 持有已关联目标，页签卸载不结束采集；UI 不直接 ScreenCaptureKit/HTTP/文件读取 |
| 可排障 Hook 事件 | M02/M10/M14/M15/M13；C36/C33 | 现有 HookUpdate→HookEventInput→repository→sessionRecords；infra 投影保留 typed facts 与受限原生文本；UI 复用分页/搜索/详情/导出，不读 stdout 或 FFI |

## 封面真实资产闭环

本机静态资源候选须来自实际游戏/Steam 资源发现，按已核对游戏标识/路径关联，不按四款固定名字硬编码。现有封面优先保留；缺少、不可读、无效图片有可见占位与原因。静态封面读取不依赖录屏权限。

无本机封面时，仅在正式“关联并接入”完成目标绑定、完整身份仍有效且录屏已允许后，捕获该目标首张有效画面并保存到托管 assets，再保存游戏封面引用。候选选中/点击窗口/取消/错误目标/旧 generation/未授权帧不能保存。封面只保存一次，不覆盖已存在资产；写失败应留占位和恢复状态，不阻断 Hook，不把失败标成有封面。重启后仍能通过真实安全资源解析显示封面；移动外部资源/移出游戏库的行为需要资源引用及回收规则审阅。不能仅加 cover 字段或用无图夹具通过测试。

本项需要的精确资产端口、DTO 和允许路径尚待 Root 审阅；不能因此新增未审 DB 字段或迁移。已核对当前 schema 只有 `games.cover_asset_ref TEXT` 引用，未找到 assets/game_assets 表或已就绪托管资产端口。引用字段可复用；安全资源读写/展示协议、单帧保存和失败恢复须由 Owner 形成精确可编译/校验合同并经 Root 审阅，不能把既有字段当作端口已实现。

## 绑定与预览状态机

- 全局连接入口→同一面板选目标/原生点选→面板内资料与自动预览→一次确认关联并开始 Hook→接入进度→候选→显式选流。
- 选择不是注入；“关联成功”“注入已请求”“进程握手”“收到候选”“正文已选”是不同事实。
- 同目标按完整身份复用，不重复 Host/注入；不同目标显示切换影响，一次确认；取消保留旧 session、正文与历史。
- 候选 lease 与 session lease 分开。已授权有效候选自动开始预览；候选变化释放旧 lease。确认后转移或建立 session owner，不因正文/全部提取/Hook 日志页签切换卸载。取消恢复原 session 预览。
- 缺权限/撤权/最小化/不可采集/退出有专属状态与最后成功时间；旧帧不能写入新 generation，不能把黑图当实时。授权后自动恢复已选有效对象。独立刷新不改游戏宽度、DPI、焦点或 Bottle 配置。

## 诊断保真与降噪

采用受限长度、去 NUL/终端控制符、脱敏的 native 诊断，保留明确来源、kind、稳定错误码、request/wire/event/session 身份。保留真实未知值；不通过猜正则把任意文本变成结构化引擎事实。警告分级按固定 Core enum：0 Console、1 Warning、2 EmuWarning、3 EmuConnected、4 EngineType；未知 kind 保留数值/未知标签。原 warning 的 Console 副本不能导致独立 warning 被删除。

Ready 的 ABI/commit/架构、Error 的 code/message、HookInserted 的 pid/address/hookcode、InjectionFinished 的架构/exitcode、引擎与安装地址必须沿现有链传达。u64 保持十进制字符串或既有安全表示，不转 JS Number。实际 DLL/版本信息由已核对资源锁与真实选择提供，未知不编造；helper exit0 不当 DLL 已加载/正文成功。

默认 Hook 视图“接入与故障”过滤或聚合低级 raw 处置噪声；全部事件/全部提取维持完整范围、顺序、计数、分页、搜索和导出。所有 warning/error/丢失/存储失败保留，汇总不得伪称无损。现有 metadata_json 可复用但具体 typed facts/DTO 改动需 Root 审契约、生成检查和存储边界回归，不能直接塞未约束 payload。

## 验证与通过边界

工作卡与 verification 仅映射现有注册检查。新的五项真实行为断言、视觉状态和 native/game adapter 覆盖仍需实现并独审；登记存在不是 ready。不得通过删真实图片、关闭录制、只看字段、沿用旧图或调整阈值掩盖差异。原游戏启动 UI 用例需用本 change 记录的合法范围覆盖修改，保留旧断言历史及修改审阅。

真实模型非本轮五修复的前置；Hook、封面、窗口预览和离线日语分词应独立完成。新包统一身份后分别跑 SR/MO/Never7/サクラノ詩10th；中文 SR 仅检验取文和安全运行，不证明日文注音。系统权限若需重启仅进行一次受控正常退出/重启，并留签名/PID/包证据；不循环重启、不重置无关权限。
