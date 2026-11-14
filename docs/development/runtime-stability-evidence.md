# 同一正式包的稳定性执行与影响闭包

本工具落实重构 SR6 的证据要求，不增加首版产品功能，不代表 153 个发布组合已经通过。真实游戏、原生窗口与睡眠唤醒尚未由本次工具开发执行；guard fixture、编译命名列表和浏览器组件测试均不继承为实机结果。

## 两个关口

`lifecycle-impact-contract.json` 显式登记关键路径、9 项义务、14 条不变量、原阶段最小检查、29 个 Rust 完整 qualified case 和 21 条组件 case 链接。路径选择来自 `check_scope` 的真实 Git diff／完整前像差异，包含新增和删除；不从开发者填写的 `covered_obligations` 推导覆盖。字面 Rust `mod`／`#[path]` 与 TS 相对导入另核对关键依赖，既有 AST/Cargo 边界守卫继续运行。

`verify.py` 每次先运行影响选择，结束后再次独立读取本次原始 Cargo target/test 行、真实 100 组件报告和已有验证器，核对当前源码、nonce、注册命令、具体 case 与无 skip。只报测试总数、检查名称或 producer 的 pass 字符串不能闭合。原 storage guard 按真实模块与路径继续取并集；M14 仍是 native-broker，M10 是 history-persistence，M16 是 credentials，不能把其他模块冒作通用存储 Owner。新文件、删除／改名、错误模块／组件、缺检查与未登记关键依赖拒绝；路径的 module_ids 与 component_ids 必须在同一 requirement 上共同命中允许的 Owner，不能借用其他需求的组件。已冻结片段保留原最小检查，最终新 `verify-refactor-stability-rc` 即使无产品 diff 仍固定全 9 项义务，补齐当前源的组合验证。

预览投递、词卡与观察器前置整合按真实改动选择维护检查，不能因辅助 change 名称而归零，也不会提前启动真实 runtime。最终 change／release 模式还必须声明 `product-packaged-stability-tests`，缺少输入或真实执行仍失败。

## 真实运行入口与前置

执行前由 Root 核对并正常退出旧 Luna，保留其他机器的 Steam 会话。先冻结最终源码并构建同源、同 Core／语言资源、同证书的 ZIP 和安装应用。运行过程中不编辑源码或更换包。工具只冷启动该精确应用一次；四款游戏由正常用户流程启动和退出，不自动登录 Steam、不切换在线、不修改游戏的窗口大小、位置或 DPI。

```sh
npm run verify -- --change verify-refactor-stability-rc --runtime-inputs .build/final-stability-inputs.json
```

输入 JSON 仅允许以下字段：

- `schema_version: 1`；`artifact` 为仓库内真实 ZIP 的相对路径，旁边保留 build-manifest。
- `application`：实际安装的 Luna.app 绝对路径。
- `profile_database`、`profile_registry`、`profile_id`：实际活动档案与数据库；工具核对 registry，不写死 UUID。
- `log_directory`：该应用的真实 log 目录。
- `games`：固定按 `sr`、`mo`、`never7`、`sakura10th` 顺序，每条具有真实的 `environment_key`、`target_exe_path`、`architecture`、`source_language`。SR 为 zh-CN，其余为 ja；樱之诗须冻结实际 Bottle，不能按标题合并两个安装位置。

不接受任意命令、fixture 输入、用户 pass、忽略用例或自填预期台词。最终执行器由代码登记，不会落入 Cargo 输出解析的默认分支。真实 runtime 采用单独 7200 秒总执行预算、每步最多 600 秒等待；不修改任何生产 deadline／队列容量。

## 人工动作与机器判断

运行器在独立输出目录写 `pending-action.json` 并输出 `LUNA_STABILITY_ACTION`。Root 通过 Computer Use 或用户正常完成选窗、推进对白、正常退出、真实系统 Sleep/Wake，再写同目录 `continue.json`。仅接受以下身份与实际窗口信息：

```json
{"execution_nonce":"本次32位hex","step_id":"当前step","game_host_pid":123,"windows":{"game":456,"main":789,"hud":790,"preview":789}}
```

窗口角色是实际 OS window ID。`preview` 可以与包含预览的主窗为同一 OS 窗口，但机器必须从真实 AXImage 的“目标窗口当前画面”描述与实际 frame 取得预览图像裁剪。整张主窗同时含正文，其 OCR 不能证明预览同步。`wordcard` 在相应用例中提供真实词卡窗口。动作回复只代表普通操作完成，绝不直接生成通过结论。

每款均验证冷发现、一次关联 Hook、显式选流、至少五条不同原文、实际游戏／主窗／HUD 对应与变化预览。MO／Never7／樱之诗另验证真实日文 token UTF8 区间、读音、词卡和固定旧对象。樱之诗执行消费者切页／主窗关闭重开、长短句自然内容自适应时保留旧词卡（不点击 HUD 或切换注音，避免合法外点关闭）、四种普通关闭动作及真实睡眠唤醒。SR 中文不能代替日语注音与词典验证。MO 启动失败保留 MO 的 blocked，不换另一游戏替代。

JS never-settle、late-finally、单 context in-flight 与受控慢消费者由同源真实 100 组件维护用例验证；正式包通过普通消费者退出／重开后的 producer 持续和可见恢复补充，不新增私有调试 IPC 或伪造 listener 数量。

## 原始证据与独立复算

- 系统观察器只读查询 PID/start/boot/实际映像 dev+inode、OS guest-code 签名、CG 窗口、AX 与真实截图／Vision OCR；不注入输入、不请求权限、不改变窗口几何。
- SQLite 使用短事务、`mode=ro`、`query_only`，包含 WAL；只取本轮新 SID 的正文、版本、原始事件、source refs、词法与 Hook 事件，不读取凭据或模型配置。
- 原始 payload 与 Host 标准输出仅保存在本机受限验收目录，用于核对 source spans、真实尾文本与持久化。生产私有 observer 始终只有固定元数据，不写台词或图片。
- 采集器 guard accepted、service committed、server delivered 与可见 UI 分层。worker_sequence 和 service_sequence 是两个独立空间；metadata-only、旧帧时间或状态标签不能代替实际 fresh frame。
- 被动 ledger 归档器复制真实元数据段，避免人工等待或系统睡眠造成两个保留段丢失。独立 validator 按 segment 的物理记录顺序核对 health 的已写入计数及真实 attempt ID 集；允许线程入队乱序和 Relaxed producer 计数瞬态，但早期 health 不充当最终完整性证明。每步取证结束与全轮最终取证均记录实际时间边界，仅在验收侧最多 15 秒只读等待边界之后两条稳定 health；必须实际看到 attempted/enqueued/processed/written 一致以及已写入 attempt ID 的完整集合，不能从 max 推断前缀。完整 live 归档保留，冻结至实际 closing health 的原始字节前缀；独立校验器逐步复核边界、对应原始 health、完整记录和 live 字节前像，末尾排队、缺尾 health、缺失最后记录或缺步均拒绝。该等待不会暂停 producer、触发业务退出或扩大生产预算。
- 自然退出要同时有 Host 正常 final snapshot／ACK／EOF 无错误、原生 owned Host 消失、SQLite stopped／全序完整回调身份、以及原 expected-target lock FD/inode 在下一个游戏启动前消失；下一个游戏成功本身不足以证明旧租约已释放。
- 真实 Sleep/Wake 需本轮 `pmset` 的 Sleep→Wake、同 boot/main/game 身份、新捕获帧与新正文。锁屏、显示器休眠、DarkWake、旧日志或只更新时间都不能通过。

缺 AX、权限、OCR、已归档原始记录、当前 SID 或真实系统事件时，相关项 blocked／failed。运行输出与计划定义分开，旧失败保留；工程守卫、同源测试与实机结果分别报告，不提升为完整首版发布。

Host JSONL2 没有 sequence；stdout 行号不能当 wire。独立尾部校验按完整 PID/address/context/context2、hook_code/name、text 比对原始 stdout、真实 final-snapshot 各流 history 的 ClientEvent.sequence 与同 SID 数据库的 ingest 顺序、event_id=SID:wire、来源 hash/Host 身份、实际 payload 与正文 source_refs/selection。wire 可有合法空洞，重复文本保留次数。控制事件按真实协议分类；仅非目标 process_disconnected 可按客户端既有规则忽略，错误／overflow／非目标正文不可过滤。每流 history 最多 100 条，若实际 text_events 超过保留 history，则缺少全量精确 wire 桥，此项 blocked，不能猜补。final-snapshot 的 shutdown ACK 只证明 Host 层，应用的终态、旧 Host 消失和目标锁 FD/inode 释放仍独立核验。

首个尚未关联 Hook／预览的 cold 检查点允许只有 begin 与两条边界之后的零计数健康记录；所有 attempted/enqueued/processed/written 及 writer 水位必须实际为零，缺 health 或非零未核销计数仍拒绝。该规则仅允许空区间结构闭合，整轮运行和后续画面／正文／词卡场景仍须各自真实业务事实，不能用空健康记录证明通过，也不为采集器人为制造启动事件。
