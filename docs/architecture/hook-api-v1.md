# Hook 后端 API v1

> 当前旧 Reader 的已实现协议，仅用于原版 LunaHost。新 Core 接入计划另立 JSONL v2，见[重建架构](client-rebuild-architecture.md#5-私有协议状态与背压)。不能把本文十回调及注入成功码用于 Core；v2 尚未实现。

实现的类型以 [`hook-contracts`](../../crates/hook-contracts/src/lib.rs) 为准。
客户端与应用层依赖此契约，不能各自复制一套有差异的 native 结构定义。

## 调用方式

宿主是单实例 Windows x64 子进程。启动参数提供固定 runtime 目录；每行标准输入是一条 UTF-8 JSON 请求，
每行协议标准输出是一条 UTF-8 JSON 事件。原生 CRT/注入子进程的调试输出必须与协议输出隔离。
原生 DLL 回调不直接调用用户的网络 webhook；应用核心在宿主事件上建立自己的订阅/回调 API。

请求示例：

```json
{"v":1,"id":"health-1","method":"ping","params":{}}
```

`v` 必须为1。`id` 为1～128个 ASCII 字母、数字或 `-_.:`；未知方法、未知字段和无效参数被拒绝。
单行请求上限为64 KiB。地址、上下文、创建 FILETIME 等64位值用规范十进制字符串传输，避免 JavaScript 精度丢失。

## 命令

| method | params | 语义 |
|---|---|---|
| `ping` | `{}` | 确认协议服务存活；不代表任何游戏已连接 |
| `status` | `{}` | 返回目标阶段和完整性是否受溢出影响 |
| `attach` | `pid`, `expected_image_path`, `expected_creation_time` | 核对 Windows 进程身份、架构后发起连接/必要的注入；响应 pending |
| `detach` | `pid` | 请求断开该宿主管理的目标；异步观察 disconnect |
| `shutdown` | `{}` | 进入关闭阶段，有界处理连接目标后退出进程 |

`expected_image_path` 必须是 Windows 绝对路径；`expected_creation_time` 为该目标的 Windows FILETIME 字符串。
同名进程或旧 PID 不能代替身份核验。宿主不提供自动提权或任意 DLL 注入路径参数。
首版同一宿主内一个 PID 只可尝试 attach 一次；断开、失败或 PID 被重用后均需新宿主实例。
应用核心用子进程实例标识及执行环境隔离事件，不能把不同实例的相同 `stream` 合并。

## 事件

| event | 主要字段 | 能证明什么 |
|---|---|---|
| `ready` | `host_arch`, `upstream_commit`, `abi_profile` | 固定运行时验证及 Host 初始化完成 |
| `response` | `id`, `result` | 指定请求已处理；attach 的 pending 不代表已连接 |
| `error` | `id`, `code`, `message` | 可关联请求或宿主阶段的具体失败 |
| `process_connected` / `process_disconnected` | `pid` | native 进程回调发生 |
| `stream_created` / `stream_destroyed` | `stream`, `hook_code`, `hook_name` | 候选流生命周期；创建附回写能力 |
| `text` | `stream`, `hook_code`, `hook_name`, `text` | 已收到该流的新原文 |
| `hook_inserted` | `pid`, `address`, `hook_code` | native 报告 Hook 已插入 |
| `injection_finished` | `pid`, `architecture`, `exit_code`, `accepted_by_helper` | 注入子程序结束；原版 helper 的接受返回值为1，不代表进程/文字就绪 |
| `log` | `kind`, `message` | 原版 Host 日志 |
| `overflow` | `source`, `dropped_since_last`, `dropped_total`, `integrity_lost` | 队列已丢事件；本会话不能作为无丢失验收通过 |

`stream` 包含 `pid`、`address`、`context`、`context2`。后三项是十进制字符串。
此结构是应用层的文本流身份，不是把 C++ TextThread 或 HookParam 直接搬进跨平台协议。

## 原生回调到事件

固定 ABI 的 `Luna_Start` 注册 Connect、Disconnect、Create、Destroy、Output、HostInfo、HookInsert、Embed、I18N、EmuInfo。
取文路径主要使用 Output/Create/Destroy 和进程/日志回调。未交付的内嵌/模拟器业务不因提供非空 stub 而宣称支持。

Output 提供 UTF-16 hookcode/text、UTF-8 hookname 和按值 ThreadParam。
回调立即复制数据，非阻塞入队后返回；协议写线程再输出事件。所有消费者都不能持有 native 借用指针。
应用层负责选流、合并、去重和会话历史；宿主不针对 Never 7 写死 MAGES2 地址或自动挑某一行。

## 首版能力边界

- 支持 Windows x64 宿主接入普通 x86/x64 Windows 目标；真实游戏覆盖需分别验收。
- 原版 x86 注入器包含 WOLF 特定校验修改分支；首版明确拒绝该类目标，纯注入路径另列后续任务。
- 固定 Host 缺少完整 Stop/Join/取消连接，生命周期以子进程为边界；不提供 DLL 卸载或进程内重新 Start。
- 只有进程内和 private 子进程管道；公共 HTTP/WebSocket、认证和远程控制由后续应用层另行实现。
- 非 Windows 平台直接运行宿主返回 unsupported，不伪造 ready/connected/text。

