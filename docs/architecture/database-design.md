# 首版数据库与字段设计

版本：候选补充1.2 · 更新：2026-10-30 · 原2026-10-19字段设计及现行DDL身份保留。第14章为13HTTP协议/21预设候选，第15章按用户本轮要求补充本机非大模型翻译；新字段、codec、关系与迁移均待复核，不代表生产已实现或用户已批准数据库细节。本轮不改DDL、schema-contract、字段CSV、旧seal或用户库。

此前[研发基线](first-product-engineering-baseline.md)只给出了 SQLite 选型、主表和持久化原则。本文补齐库的边界、实体关系、字段约定、不可变版本、授权事务、清理与迁移规则。完整字段以 DDL 和自动生成字典为准，避免在多处手写表数、字段数或重复维护字段定义。

## 1. 设计依据与交付物

依据为[首版范围 FP-01～12](../product/first-product-scope.md)、[游戏库与关联 PRD](../product/game-library-window-prd.md)、[模型配置 AI-CFG-01～16](../product/model-configuration-prd.md)、[按需精读 PRD](../product/ai-reading-prd.md)、[词法与模型边界](japanese-study-and-llm.md)及[工程组织](engineering-layout.md)。本设计继续使用 `rusqlite bundled SQLite`、单 writer、Repository 端口和独立 CredentialVault；不引入新的 ORM、数据库服务或前端 SQL。

| 交付物 | 维护方式与用途 |
|---|---|
| [profile.sql](database/profile.sql) | 用户库中的游戏、取文、阅读、任务、结果、偏好和运行授权设计 |
| [config.sql](database/config.sql) | 同一用户库中的配置快照、用途、模型、模板、术语、凭据引用与独立测试记录设计 |
| [dictionary.sql](database/dictionary.sql) | 独立只读辞书索引的资源构建设计 |
| [字段字典](database/field-dictionary.md) · [CSV](database/field-dictionary.csv) | 从上述 DDL 派生，逐字段展示类型、可空、默认值、键、约束与中文含义 |
| [表目录](database/table-catalog.csv) · [机器元数据](database/schema-metadata.json) | 表的职责、Owner、FP 追溯及 SQLite 实际解析到的结构 |
| [实体关系图](database/relationships.svg) | 按实际外键生成，可横向查看；只读辞书与用户库保持分区 |

SQL 位于架构目录，是供设计检查的 DDL，**不是产品已安装的 migration**。本轮不创建或迁移用户 `reader.sqlite3`；后续授权产品实施时，AP 根据复核后的设计在 `crates/app-infra/src/storage/{database,migrations,queries}/` 编写带版本与数据转换的真实迁移，再由既定测试规划验证。

## 2. 库、文件和进程边界

| 数据位置 | 保存内容 | 访问与职责 |
|---|---|---|
| `profiles/<profile_id>/reader.sqlite3` | 一个配置档的游戏、来源、历史、版本化配置、结果、缓存、凭据引用与控制状态 | M10/AP 管 writer 和事务；M01/M03/M04/M05/M09/M11 通过端口使用 |
| 随包只读辞书 SQLite | 词条、检索词形、中文释义、明确标记的英语补充、来源与许可 | M08/JA 只读查询；构建流程生成并核对整库一致性，用户运行不写入 |
| Keychain / Credential Manager | API Key 等实际认证材料 | M16/PL 经 CredentialVault 读写；业务库只持引用、版本、脱敏标签和生命周期 |
| `profiles.json` | 配置档索引与启动选择 | 平台配置协调；不是全量模型配置或阅读历史的 JSON 数据库 |
| 平台偏好 | 窗口几何等系统窗口状态 | M12/PL；不存正文、模型配置或凭据 |
| 共享锁与目标租约目录 | 配置档独占和完整目标身份的跨进程占用 | HK/AP/PL 协调；单档数据库唯一索引不能证明跨档或跨实例目标独占 |
| 诊断与测试运行目录 | 脱敏诊断、运行证据及归档索引 | M15/QA；不是阅读历史，也不默认加入可分享报告 |

每次打开库同时核对路径所指配置档、`profile_meta.profile_id`、schema 版本和实际资源清单；不能仅从文件名推断身份。切档要停止旧档任务、递增运行代次并切换 Repository，不能跨库直接 JOIN 以绕过配置档隔离。共享系统凭据须有明确授权；同机安全库存在相同服务名不产生新档权限。

React 只接收应用 DTO，不能读取 SQL 行、Keychain 引用或连接句柄。Tauri 是组合与 IPC 适配层，领域层不依赖 `rusqlite`。Hook broker 与 `luna-hooks-core` 不链接数据库、模型或辞书；Hook 回调只复制并发送有界事件，数据库压力不能反向堵住游戏线程。

## 3. 字段与版本约定

| 约定 | 设计含义 |
|---|---|
| 稳定 ID | 业务 ID 使用非空 TEXT，由 Rust 产生并验证格式。显示名、窗口标题、EXE 文件名和裸 PID 不充当主键 |
| 时间 | `*_at_ms` 为 UTC Unix 毫秒 INTEGER；NULL 表示尚未发生或未知，0 只有实际定义为零时才使用 |
| 布尔与枚举 | 布尔 INTEGER 受 0/1 CHECK；状态使用 TEXT 枚举。STRICT 类型不代替业务状态校验 |
| 哈希 | SHA-256 小写十六进制 64 字符；SQL 检查格式，Rust 以指定规范编码重新计算真实内容摘要 |
| uint64 | Hook 地址、context、subcontext、wire sequence、进程创建时间使用规范十进制 TEXT，保持完整位宽；不能经过 JavaScript Number 或 SQLite 有符号 INTEGER 截断 |
| 内部顺序 | 本地 ingestion / unit ordinal / revision 用有界正整数；排序用明确本地顺序键，不能把变长 uint64 十进制文本直接按字典序作数值排序 |
| JSON | 仅存白名单且版本化的结构，JSON TEXT 加 `json_valid` 和预期顶层类型。用于查询、外键、唯一性或授权的关键字段独立建列 |
| 文本与跨度 | 原文以 UTF-8 TEXT 保存；所有持久化半开跨度为 `[byte_start, byte_end)`，相对固定原文版本。UTF-16 / DOM 索引仅为 UI 投影换算 |
| 缺省与空 | SQL NULL、空字符串、JSON null、未发送参数、未知用量分别表达；不通过一个空值混用多种语义 |
| 作用域 | `scope_key` 明确为 `global` 或 `game:<game_id>`，以 CHECK 与复合键约束，避免 NULL 全局键绕过唯一性 |

以下版本分别承担不同职责，不能互相替代：

| 版本/身份 | 对应用途 |
|---|---|
| `schema_migrations.version` | 库结构与迁移兼容性 |
| `games.row_revision`、目标 row revision | 用户可编辑记录的乐观并发 CAS |
| `config_revisions.revision_id` / head revision | 不可变整体配置快照及草稿、已应用指针的并发控制 |
| `content_hash` | 完整非秘密快照内容身份，显示名变更也可能改变 |
| 配置语义 hash / request semantics hash | 前者标识有效请求配置；后者进一步包含确切原句、上下文、术语、所选词等发送材料 |
| `text_revision` | 同一阅读单元不可变原文版本 |
| `authorization_epoch` | 用途授权或请求语义发生变化时作废旧任务 |
| `runtime_generation` / server instance | 配置档重新激活、切换或服务重启的运行隔离 |
| `profile_meta.commit_revision` | 成功业务事务的本地提交次序 |
| projection revision | 某个窗口读模型的快照和 delta 次序；不等于数据库提交号，也不在重启后重放旧 Channel |
| 资源 manifest / token schema | Core、语言 worker 与辞书的确切资源和契约身份 |

## 4. 游戏、取文与阅读来源

### 4.1 游戏记录与本次目标分开

`games` 是稳定游戏库条目；`game_targets` 保存当前可编辑启动入口、工作目录、结构化参数、真实 Bottle 引用、规范 EXE 路径和架构。首版每条游戏一个当前目标配置，启动路径与实际注入 EXE 可以不同。环境和规范路径由 Rust 平台解析器核验，不能把标题匹配或待解析快捷方式当成已核验目标。

活跃条目按 `environment_key + target_path_key` 去重，不同 Bottle 同路径独立。`verification_state=verified` 只说明目标已核对，不代表 DLL 加载、Hook 完成或已经收到文本。编辑目标推进 row revision；已有 `sessions` 保留连接当时的完整身份，不被库条目编辑反写。

移出库写 `archived_at_ms`，不删除游戏文件和已有历史，也不使另一条目继承旧活动会话。窗口关联句柄属于运行观察，不能持久化后在重启时直接恢复为有效授权。

### 4.2 完整流身份与显式选流

`sessions` 固定 Hook 或手动输入来源。Hook 会话保存环境、真实路径、PID、创建时间、位数、broker 实例、实际 Core 资源及身份摘要；手动会话不伪造 Hook 字段，可无游戏记录。游戏进程重启或 broker 重建产生新会话，不能复用旧 PID 的历史身份。

`capture_sources` 保存 Host 实例、Hook 地址、context、subcontext、code、编码及完整身份摘要。Hook 名称用于展示，不能按 `TextOutW` 名字或列表第几行自动选择正文。`source_selections` 使用会话内递增选择版本，记录用户确认的流；临时选择与明确保存的匹配快照分开。

`raw_events` 保存不可变事件身份、wire sequence、本地 ingestion 次序、payload 长度/摘要与完整性状态；大体积原始字节拆到 `raw_event_payloads`。`source_refs` 将每个正文版本关联到同会话、已选流的来源事件和跨度。合法重复台词不能被简单文本去重丢掉；丢失事件要有 `integrity_loss` 等可观察记录。

### 4.3 固定正文、词法与历史

`reading_units` 是稳定阅读单元，`reading_unit_revisions(unit_id,text_revision)` 是不可变原文版本，保存原文/hash、语言、净化分段规则版本和当时选流版本。模型、词法和人工修正均固定到该版本，不能在回调完成时重新读取“当前句”决定归属。

`lexical_snapshots` 记录真实语言资源、分析器和 token schema；`lexical_tokens` 保存词面、词元、读音、词性和 UTF-8 跨度。SQL 检查词面字节切片和不重叠，应用仍核对 UTF-8 边界、token 个数/顺序、覆盖规则、未知词及资源版本。`is_known` 指词法器是否认识该词，不等于辞书已有中文释义。

历史查询使用稳定复合游标：游戏/会话过滤、固定查询边界、时间或顺序加稳定 ID；不只使用 OFFSET 在并发写入、清理中翻页。当前句、固定精读对象与历史分页各有投影；固定对象在生命周期内应保留确切原句及必要资源/释义快照，不能因后台保留清理悄悄换句或空白。

## 5. 配置快照、继承与应用

### 5.1 不可变快照和两个 head

`config_revisions` 是一个配置档的整体非秘密配置快照；`connections`、`models`、`purpose_bindings`、有序 `binding_engines`、参数、模板与术语都携带同一 `revision_id`。稳定实体 ID 可跨版本相同，精确历史引用必须同时带 revision，不能 JOIN 当下模型或连接反算旧请求来源。

草稿与已应用使用独立 `config_heads`，各有 CAS 版本。保存草稿允许字段缺项，只推进 draft；不移动 applied、不自动发请求、不把发现模型列表或保存成功当网络测试成功。应用前检查目标用途的可运行性，生成完整候选快照后原子切换 applied。

“应用当前实体”应以当前 applied 为底，合入用户明确选择的变更及经确认的依赖闭包，再生成新整体快照；不能把整个 draft 中其他用途的未应用编辑顺手启用。并发窗口的 expected head revision 不匹配时返回冲突，保留用户编辑，由用户重新核对后重试。

### 5.2 封存与受控 GC

配置 snapshot 和凭据池 revision 先在同一事务内装配 header 及全部子行，校验完整引用与规范内容 hash，然后分别写 `config_revision_seals`、`credential_pool_seals` 封存。head、任务、独立测试以及实际连接引用只能使用已封存版本。封存后不允许后插入、UPDATE 或任意删除子行，以免“只禁止 UPDATE”却仍能改写已应用版本。

新编辑产生新版本；封存不能撤销，也不能用同一个 ID 删除后重建。`config_gc_marks`、`credential_pool_gc_marks` 是受控回收的持久 tombstone，不是可临时开关的绕过标记。GC 先核对该版本不被草稿/应用 head、运行控制、历史任务、独立测试等实际内容引用，才标记并删除可回收的重表子行；header、seal 和 tombstone 保留，保证 ID、内容身份与回收事实仍可追溯。

`config_revisions.parent_revision_id` 只保留来源链，不单独阻止重表子行回收；凭据池的回收还须等待所有配置连接和测试凭据成员引用解除。精确删除次序、拒绝条件和字段以 DDL 为准；应用 GC 还须核对内存中在途 lease、固定对象与授权生命周期。数据库无外部 FK 引用不等于可删除系统秘密或当前进程仍在使用的资源。

### 5.3 先解析有效 binding，再派发

用途分别为 `translation`、`grammar`、`context_word`。同一整体版本内，同 scope 同用途只有一个 binding；翻译可有多个有序引擎项，两个学习用途各至多一个。连接可用意图、用途启用、引擎启用、实际服务能力、即时安全停用和测试新鲜度分开表达，模型没有额外的启用开关。

解析按以下步骤进行：

1. 用实际游戏上下文选择该游戏用途；没有游戏覆盖或 `binding_mode=inherit` 时解析到同版本 global 用途。
2. global 必须 `override`。游戏 `override + enabled=0` 是明确关闭，不能继续回退到 global。原始 inherit 行的 enabled 值不作为有效启用状态。
3. 输出明确的有效 override binding、授权 scope、确切引擎与来源链，供 `purpose_runtime_states` 和任务固定引用。游戏来源仍通过 session/unit 追溯，不能把授权 scope 当作游戏身份。
4. 每个引擎逐键解析参数：适配器默认 → 模型默认 → global 用途/引擎 → 游戏显式覆盖。`inherit` 沿上层，`set` 保留实际值（包括 0），`omit` 阻断继承且不发送；可选 JSON null 只有适配器明确允许时才作为真实值发送。
5. 模板按实际用途、引擎和模型默认解析，验证协议、模型、语言和变量；翻译模板不冒充文法结构契约。术语、上下文及语言方向形成确切材料范围。
6. 校验实际协议能力、地址与认证、所有启用项、预算/超时/并发/显式重试回退，再计算有效配置语义和请求语义。

游戏从 inherit 切到 override 或关闭时，应用必须在同一 writer 事务内切换有效配置，并按原会话/游戏归属作废该游戏受影响任务；不能因旧全局闸仍启用就接收该游戏旧请求，也不能误停其他游戏的全局用途任务。SQL 授权闸不能替代这段继承解析和受影响对象选择。若后续实现采用物化有效配置，仍须保持同样作用域和来源语义，不能默默改变继承结果。

`purpose_runtime_states` 保存当前已解析有效 binding 和授权 epoch；每次更新只允许维持原值或增加 1，不可回退或任意跳号。启停、有效 binding 或有效语义变化必须增加 1，并在同一 writer 事务作废对应未完成任务。仅改显示名或界面偏好、请求语义不变时允许保留 epoch，不为无关编辑取消任务。

### 5.4 规范 hash 与独立测试状态

规范编码由 Rust 的版本化 canonical 结构决定，固定字段、字符串/数字形式、map 排序和省略规则；不能用随机 map 序或原始 JSON 文本临时生成缓存键。请求语义包含实际协议/端点/模型、有效参数、输出契约、模板、术语、允许上下文、所选词句、语言、凭据/池版本与相关资源身份，不含显示名、窗口字号或布局。

`test_records` 记录用户主动执行的 local / connection / model / purpose 具体阶段、精确配置、输入范围、语义指纹、输出校验、时间与真实终态；不进入正文历史。`test_record_credentials` 单列本次实际触及的 Key/池成员。测试一把 Key 不能提升整个池；连接测试不能提升文法；更改请求语义后派生为 stale，保留旧记录，不重写旧结论。样例输入优先，选择真实台词必须是明确固定当前句及授权范围。

## 6. 任务、尝试、结果与取消事务

`request_tasks` 固定原句版本、session、用途、有效 binding、配置、引擎、profile generation、server instance、授权 epoch 和输入语义；从 queued 开始。`request_attempts` 新建只能为 started，记录有限的 initial / retry / fallback / cache 路径、实际引擎与凭据版本、用量和终止原因。任务终态不复活；重启后中断请求标为失败/失效，不能自动重新付费发送。

初次和重试使用原任务引擎；fallback 必须来自冻结调度策略的显式授权，不因同 binding 中存在另一个引擎就默认允许。多翻译器独立并发产出不是失败回退。未知 token 用量保留 unknown，不写成实际 0；取消只能保证本地停止提交，不推断远端未执行或未计费。

| 事务边界 | 必须原子完成的内容 | 事务外操作 |
|---|---|---|
| 入库/编辑/移出 | CAS 验证、记录修改、commit revision、operation receipt | 文件选择、平台目标核验先完成；不删除游戏文件 |
| 原文持久化 | 事件身份/payload、阅读版本、source refs 和关联提交 | Hook 回调复制、规则计算和语言分析不持写锁等待 |
| 保存/应用配置 | 装配并封存快照、head CAS；应用时同步授权 epoch、受影响任务失效及回执 | 语法/能力/材料预检先完成；网络不在事务内测试 |
| 请求派发 | 固定已授权任务/attempt 身份并再核对用途、引擎、连接与凭据 | 取得短期发送 lease 后读取秘密、执行 HTTP；发出前复核控制代次 |
| 完成提交 | 复核任务、attempt、当前 profile/server/generation、epoch、启用项与凭据；写终结果、缓存和终态 | 解析、大小/结构/引用校验先完成；提交后发布 projection |
| 取消/停用/撤销 | 持久化禁止或任务终态，并作废相关 queued/running 任务 | 通知网络取消/worker 终止；失败也不恢复本地写权限 |

完成与取消经过同一 writer 排序。取消先提交，后来到达的响应不得进入译文、精读正文或缓存；完成先提交，合法历史已经存在，之后的取消不会伪称结果从未发生。仅换到下一句且未取消、未语义失效的旧请求可保存到所属旧原句，不能覆盖新句投影。

`translations` 和 `study_results` 只保存完整、有效、已授权的终结果。半段流式内容只在运行内存显示，不混入完成历史。结构化学习结果保存 schema、选区、引用和歧义；纯文字模式不画出未经验证的精确 token 高亮，也不保存隐藏思维链。

`corrections` 使用独立修正 ID、版本和 CAS 基值，不能被后台机器新结果覆盖。`cache_entries` 保存已校验的独立派生结果，不以悬挂 FK 永久阻止历史清理；命中后仍核对当前权限，不能让缓存绕过用途停用或秘密撤销。人工修正优先级由明确偏好决定，不能反写成模型原始输出。

`operation_receipts` 与业务提交同事务保存。相同 operation ID + 相同请求摘要返回既有回执，不重复本地业务；相同 ID + 不同摘要拒绝。该机制不单独证明 Hook 注入、HTTP 或 Keychain 等外部副作用 exactly-once，外部过程仍须身份、lease、状态协调和恢复协议。

## 7. 凭据、池版本和跨存储恢复

`credential_refs` 保存不可变秘密身份、vault 定位、脱敏标签与 pending / active / revoked / cleanup_pending 生命周期。替换 Key 生成新 ID，不能改写 vault_ref 后让旧任务悄悄使用新秘密。`credential_pool_revisions`、members 和 seal 固定成员顺序、启用意图、权重与策略；新池版本不自动替换旧已应用连接。

新增流程先在数据库登记可识别 pending 意图，事务外写系统安全库，再以短事务承认可引用状态和明确配置引用。任一步失败或崩溃都能识别未引用 pending 项；恢复清理必须先检查真实引用与在途 lease，不删除仍被已应用或任务使用的秘密。pending 从不授权外发。

撤销先持久化 revoked、推进生命周期版本并停止相关任务，再尝试删除系统秘密。系统删除失败显示待清理，仍保持禁止；不能把 state 改回 active 恢复权限。`connection_runtime_states` 是跨配置版本的即时停用闸，恢复必须明确操作，不能随保存草稿或重启连接自动恢复。

SQL、JSON、模板、错误、日志、缓存、测试记录和导出均不得保存明文 Key、认证 Header 或带凭据 URL。数据库备份不含系统秘密，但仍含正文及本机路径，属于私人资料。跨设备导入只恢复脱敏配置与历史，凭据引用重新关联和明确启用后才能请求；不能把“导入完成”显示为服务已可用。

## 8. 内置辞书和资源版本

只读辞书以 `dictionary_meta` 标识资源 manifest、index schema 与规范化算法。`dictionary_sources` 保存原数据快照、hash、来源 URL、包内许可引用和必须显示的归属；URL 是来源说明，不意味着运行自动联网。

`dictionary_entries`、`dictionary_forms` 和 `dictionary_senses` 保存词元/读音、按固定规则生成的检索键、排序、释义语言、来源定位。中文基础 `zh_base` 与英语补充 `en_supplement` 明确区分；UniDic 词法资源不等于中文释义库，不能将英语释义伪称中文命中。未知词保持可解释的未命中，不回退到样例词典冒充实际资源。

用户库与资源库没有跨库 FK。词法历史及学习结果通过资源 snapshot/hash/schema 与必要内容快照追溯；旧资源下的 token 不在升级时被无声重解释。资源构建工具验证最终整库的 content kind、gloss language、查询键、来源与许可一致性，运行连接只读打开。用户输入不写入随包辞书；首版不借此扩展用户编辑辞书功能。

## 9. 清理、保留、索引与恢复

### 9.1 按数据性质清理

| 数据 | 保留与删除规则 |
|---|---|
| 游戏与目标 | 移出为归档，历史引用继续有效；不删除游戏文件 |
| 原始 payload | 依显式容量/时间政策按批清理；保留 event 身份、摘要、长度和完整性，不级联删除阅读历史 |
| 阅读原文、译文、学习与人工修正 | 按用户明确的历史清理范围删除；先检查当前/固定对象，明确保留、快照或解除策略，不误删仍显示对象 |
| 配置/凭据池 snapshot | 受控 GC 先核对无外部引用，标记 tombstone 后删可回收子行；header、seal、tombstone 保留 |
| cache | TTL、容量与 LRU 有界清理；删除不抹掉人工修正和原始历史 |
| test_records 与 receipts | 按独立保留政策管理；当前语义需要的最新判断和幂等窗口内回执不可提前删 |
| 系统秘密 | 按生命周期和真实引用独立清理；删除配置快照不等于允许删除 Key |
| 诊断与证据 | 私有归档策略单独控制；不以数据库历史保留政策擅自上传或删除证据 |

raw payload 删除后，来源页面明确原始字节已清理，不能声称仍可完整回放游戏原始事件。历史清理可保留最小身份与来源摘要，但不能以“清理”原地改写 immutable 正文或模型结论。

DDL 索引覆盖活跃游戏、游戏会话、来源事件顺序、历史游标、任务取消范围、用途/模型/连接反向引用、测试语义与时间、凭据清理、cache expiry/LRU。实际查询需在真实数据量上验证 query plan、分页和资源占用；列出索引不等于性能通过。

### 9.2 单 writer 与失败反馈

用户库使用专用单 writer 执行器和有界队列，连接逐一启用外键，配置经冻结的 busy timeout、WAL 和同步策略。写事务短小，不持锁等待 HTTP、Keychain、游戏、worker 或 UI。读连接/快照按查询生命周期管理，慢读不能无限阻碍 checkpoint 或回收。

busy、只读、磁盘满或损坏向应用返回稳定阶段错误，保留已提交版本。未能落盘的新编辑保留在 UI 草稿并提示未保存，不能在同一写盘故障中又承诺已持久化“失败草稿”。原文可见与历史成功保存分别表达，队列饱和或完整性丢失不能静默冒充全量历史。

### 9.3 迁移、备份与回退

迁移前核对数据库所属档、当前版本、受支持升级跨度、迁移源 hash、可用空间和恢复入口，再创建一致备份。包含 WAL 的在线库使用 SQLite Online Backup API 或经过验证的等价一致性流程；不能只复制主 `.sqlite3` 文件当作完整备份。备份元数据记录 schema、包版本、profile、提交边界和文件摘要，访问与保留政策独立冻结。

真实迁移按有序版本执行必要数据转换，事务内更新结构、约束和 `schema_migrations`；完成后执行完整性、外键与业务不变量检查再切换。失败保留原库及备份，不写“迁移成功”记录。恢复在新目标路径验证后切换，保留失败现场，避免原位覆盖唯一副本。

遇到未知较高 schema，旧程序拒绝写入并提供恢复入口；降级不直接让旧包打开新库。不可逆转换的回退通过兼容备份恢复，不承诺任意 down migration。Keychain 不在 SQLite 事务和备份里，恢复后实际秘密访问与授权另行核验；签名或升级后的安全库访问必须实机测试。

## 10. SQL 约束与应用校验的分工

| SQL / 设计 DDL 负责 | Rust 应用与资源/平台适配负责 |
|---|---|
| STRICT、NOT NULL、CHECK、PK/UNIQUE、同版本复合 FK | ID 格式、真实 SHA-256、范围与大小/深度、跨字段用途语义 |
| 配置/池封存后子行写入拒绝、不可变记录与受控 GC 条件 | snapshot 构造、有效引用闭包、canonical 编码、GC 内存 lease 与生命周期 |
| 同会话来源、已选流引用、原文长度、token 切片和重叠 | 原始编码转换、UTF-8 边界、全量 token 对齐、UTF-16 投影、资源质量 |
| head CAS、用途 epoch 单调、任务合法状态、终结果授权闸 | 全局/游戏继承解析、单实体应用闭包、网络派发前复核、取消/停用传播 |
| 凭据元数据身份不变与生命周期单调 | 真正 vault 操作、权限、跨存储崩溃恢复、秘密脱敏和显式共享授权 |
| 结果来源的精确任务/attempt/config 外键 | retry/fallback 是否得到冻结策略授权、协议 body/headers 是否与计划一致 |
| JSON 有效与顶层类型 | 参数白名单、保留字段、模板变量、endpoint/代理安全、模型能力、结构化引用和预算 |
| 迁移记录、约束检查和 SQL 事务原子性 | 真实文件备份、磁盘满、崩溃故障点、签名与用户可见恢复 |

直接修改数据库文件或禁用外键/触发器不属于受支持写入路径。SQL 防线用于减少错误传播，不能替代应用权限、只读资源打开方式、文件访问控制或真实运行测试。上述规则的产品实现仍待授权研发后交付。

## 11. 现有测试如何验证本设计

不新增产品用例编号；沿用[测试计划](../development/first-product-test-plan.md)及其唯一登记。128 个用例定义、153 个执行组合仍全部 `not_run`。本轮隔离内存 DDL 正反例和派生一致性属于工程设计检查，不将对应产品用例标绿。

| 设计重点 | 现有测试 ID | 实施后的证据 |
|---|---|---|
| 原子提交、WAL、迁移和存储压力 | TP-01、TP-02、TP-03、TP-30 | 真实临时文件库、故障点/恢复副本、前后 hash、完整性/FK/业务断言与用户反馈 |
| 秘密 pending、撤销、升级访问 | TP-04、TP-05、TP-24、MC-13、MC-16、MC-19 | 隔离安全库前缀、真实删除失败/崩溃恢复、撤销后无外发/无正文提交 |
| 幂等、CAS 与配置语义 | TP-08、TP-10、MC-01、MC-08、MC-11、MC-12 | 并发操作 barrier、receipt/heads/epoch 变化、请求指纹与实际 body 对照 |
| 完整目标/来源、跨档和读投影 | TP-06、TP-07、TP-09、TP-27、MC-21、GL-07、GL-08、GL-09、GL-10 | 真实进程/锁、会话和流身份、旧代次拒绝、游戏库编辑后历史快照保持 |
| 取消竞争、迟到、cache 与人工修正 | TP-14、TP-16、MC-12、MC-20、CR-05-LATE、CR-13-STREAM-REVISION | 控制完成/取消顺序，数据库结果/缓存数与 UI 归属，人工版本不被覆盖 |
| 参数/模板/调度与测试范围 | TP-15、MC-03、MC-04、MC-05、MC-07、MC-09、MC-10、MC-15、MC-22 | 精确配置、凭据成员和用途记录；独立阶段测试与真实网络材料/能力检查 |
| 字节跨度、真实离线辞书与 worker | TP-17、TP-18、TP-19、TP-26、CR-12-RESOURCE、CR-12-TOKEN、CR-12-OFFLINE | 随包真实资源/hash/许可、Unicode 语料、词条来源、离线首启与 worker 故障 |
| 历史分页、固定对象、保存失败 | TP-25、CR-08-CONFIG、CR-08-HISTORY、CR-12-PERSIST、CR-13-EXPLAIN-UI | 并发插入/清理时游标、固定旧句与 UI 编辑保持、原始失败和重试回执 |

设计层检查使用 `python3 scripts/quality/database_design.py --check` 与 `test_database_design.DatabaseDesignTests`；根入口以 `npm run verify:database-design` 为准。生成字典、CSV、元数据和关系图必须与实际 DDL 一致，新增/变更约束需要合法正例及违规反例。严格拒绝所有输入并不能证明约束正确。

维护归属：AP 主责用户 schema、事务、查询和迁移；AI 主责任务/配置语义与模型结果；JA 主责 token/辞书和资源 schema；PL 主责 vault、配置档锁与文件恢复；QA 维护设计守卫、用例映射和真实运行证据。DDL 变化通过新 OpenSpec change 明确影响，不能手改自动派生字典迁就 checker。

## 12. 实施前仍须冻结的项目

数据库结构草案不替代 B0 运行政策。以下由相应 Owner 提出候选、结合受控样本测量并明确复核后冻结：

- SQLite/rusqlite 确切发布版本与构建选项，foreign keys、WAL、同步、busy timeout 和连接生命周期配置。
- 单事务/批次、写队列、正文与 JSON 的容量/深度上限，以及达到边界时可观察的拒绝、背压或完整性反馈。
- raw payload、历史、cache、test records、receipts、诊断和备份的容量/时间/归档政策；固定对象与外部引用阻挡清理时的处理。
- canonical hash schema、各 JSON 白名单、参数能力、模板输出、词法/辞书索引版本及合法 fixtures。
- 首版是否迁移旧 PoC 数据、实际受支持版本跨度、备份和恢复入口；不得仅因本设计有 migration 表就默认承诺旧数据全量兼容。
- 原生安全库与跨档共享授权、受控制品存储、敏感正文及本机路径的导出/备份访问和保留政策。

这些尚未冻结的条件保持待准备或待评审。新增数据库设计待复核、整稿待最终评审、产品真实 capture/runtime adapter 尚未实现及产品回归未执行的状态分别保留；本报告不能给出首版上线通过结论。

## 13. 字段边界与存储模块的可执行验收

新增规范由用户明确要求：存储模块验收与字段边界一并纳入，结构一致性必须由程序校验。当前仍是工程准备；实际Rust Repository、迁移和存储产品adapter没有实现，不能以本节守卫通过提升产品状态。

### 13.1 两种独立的结构来源

[冻结schema契约](database/schema-contract.json)是期望值；`storage_schema.py`读取候选库实际 `sqlite_schema` / PRAGMA为实际值。不能由同一份生产migration同时生成期望和候选后自证一致。契约创建/更新必须在有原因的新OpenSpec变更中显式执行 `--freeze` 并更新基线；默认检查只比较，绝不自动刷新。

比较覆盖表集合和额外对象、列顺序/类型/可空/默认值/主键、STRICT/WITHOUT ROWID、复合外键列顺序与父列/动作、唯一/部分/表达式索引及collation/排序、CHECK/生成表达式/延迟约束、完整触发器与view。SQL仅对空白、注释和未加引号的大小写做保守规范化；不改字符串字面量，不声称任意两份SQL等价。无法安全归一的差异留待显式审查，不能静默忽略。

程序以只读方式检查实际数据库，额外执行完整性/FK数据检查；不写数据、不运行迁移、不checkpoint或复制唯一用户库。用于比较的生产候选必须由实际adapter在隔离输出目录运行真实Rust迁移产生；设计DDL克隆仅用于比较器守卫测试。数据库的 `user_version`、迁移账本和辞书元数据的真实版本归存储adapter按PRE-14冻结规则另行核验，结构相同不等于迁移路线执行过。

### 13.2 每字段、每CHECK有可执行覆盖义务

[字段边界登记](database/field-boundaries.json)逐一列出所有字段的稳定ID、原声明指纹、合法上下文与实际探针；[合成fixtures](database/boundary-fixtures.json)不包含真实游戏或用户数据。新增/删除/改字段、缺探针、未知规则、修改预期、登记与DDL不符时，程序拒绝；检查阶段不自动补测试或改预期。

- 每个字段均验证合法前/后对照、非空合法值、错误SQLite存储类、NULL策略和适用的默认值真实省略行为。固定类型不等于应用输入类型：SQLite可无损转换的值如实作为转换正例，不能伪称被拒绝。INTEGER PRIMARY KEY显式NULL可能生成rowid，须由未来Rust输入验证单独拒绝，不能拿SQL NOT NULL文字当运行结果。
- 哈希、规范十进制uint64、JSON全文/形状、枚举及已声明数值边界使用独立语义预期；已知非法输入即使被SQLite接受，也不能生成“合法预期”。NUL首/中/尾、空串、Unicode、上下界及越界均属于适用探针域。容量/长度等尚未冻结的产品政策保持pending，不编造业务上限。
- 每个原CHECK需要真实合法控制与目标反例。能独立触发者须用逐谓词真值证明其他CHECK为true/NULL；逻辑冗余者登记明确的蕴含关系、有限类别/数值证书和联合反例，不将冗余条件冒称孤立覆盖。
- 字段层使用原始CREATE TABLE及原唯一索引建立隔离投影，保留原列/表CHECK/PK/UNIQUE；不装业务trigger，外键关闭，避免不可变快照或外键先报错遮盖字段类型。诊断谓词真值可以在独立只用于求值的连接中执行，真正正/反写入连接始终启用CHECK。此层**不证明完整FK、trigger或生产迁移已验收**；完整DDL业务正反例和生产存储验收独立保留。

本轮边界审查实际发现SQLite在部分TEXT长度、GLOB和JSON解析中可接受NUL后缀，使“规范u64/固定小写hash/JSON全文”与应用理解不一致。字段设计已对这些规范族追加拒绝嵌入NUL的CHECK，并保留独立拒绝预期；不无依据禁止自由正文中的字符。SQLite的严格类型和INTEGER PRIMARY KEY规则参见[官方STRICT说明](https://www.sqlite.org/stricttables.html)，外键及NULL语义参见[官方外键说明](https://www.sqlite.org/foreignkeys.html)。

### 13.3 存储实际验收的硬性入口

[存储验收登记与说明](../development/storage-acceptance.md)将既有TP/MC/CR组合映射到编译映射、首次建库、全部支持迁移路线、拒绝/回退、WAL备份、事务故障、busy/只读/磁盘满、凭据竞争、取消、缓存修正、历史清理/分页、档隔离和只读辞书子检查。子检查不新建或改变原128/153计划身份。

生产adapter只能在代码登记；执行器必须实际编译并运行锁定的 `app-infra` Rust测试目标，核对真实断言marker、无skip、源码/包/SQLite版本、迁移来源与候选库hash，再逐库调用结构比较器。支持升级/拒绝/恢复的每条路线要有独立断言和真实before/after/备份身份，不能只把路线ID写进JSON。实际产物/版本/支持矩阵缺失时拒绝验收；声明的名称、stdout成功文字和文件hash都不能单独代替实际运行。

```sh
# 可立即执行：设计结构、逐字段边界、存储子验收登记
npm run verify:storage-contract

# 实际产品存储验收：当前缺正式adapter和冻结迁移矩阵，应非零并返回blocked
npm run verify:storage

# 统一工程门禁亦包含设计检查与守卫正反例
npm run verify -- --change enforce-storage-schema-and-boundaries
```

开发态实际修改存储/相关领域或显式声明存储模块时，必须带已注册的真实存储检查并由总门禁调用；发布态无条件要求。没有adapter时，设计/工具change可检查通过，产品存储验收仍为blocked，153产品组合继续not_run。不得通过预填receipt、缺项跳过、换成设计DDL或放宽schema/边界预期绕过。


## 14. 全量模型服务注册：候选字段与关系（2026-10-30）

本节细化[主技术稿第5章](japanese-study-and-llm.md#5-翻译引擎与模型服务设计)，范围为13协议/21预设，腾讯TMT/TC3排除。**以下是候选字段/迁移设计，尚未更新现行DDL、schema-contract、字段CSV、用户数据库或生产DTO。** 现有字段/边界守卫通过仅证明旧结构，不证明本节已实现。整体架构确认不自动批准新增字段；实现前独立审阅并冻结新迁移与字段合同。

### 14.1 AdapterBinding采用独立候选列

此前调研提出过单个adapter_binding_json字段。本稿按第3章“授权/查询/关系关键字段独立建列”的规则收敛为以下方案；DTO仍用Connection.adapterBinding，旧JSON提案不再作为并行实施目标。用关系字段生成DTO，不让UI读SQL。

| 所属表 / 候选字段 | SQLite类型与边界 | 关系 / 语义 |
|---|---|---|
| connections.protocol（既有） | 非空TEXT；保留稳定协议ID，迁移后做ID格式而非逐家枚举CHECK | 与完整adapter key对应；不引用可卸载运行目录的FK |
| connections.adapter_binding_schema | INTEGER NULL或正整数；与下三列成组 | 已识别旧codec允许全NULL；新应用配置必须显式schema版本 |
| connections.adapter_contract_revision | INTEGER NULL或正整数 | 与revision_id/connection_id/protocol共同确定协议行为 |
| connections.compatibility_profile_id | TEXT NULL或受限非空ID，无NUL | 与profile revision组成精确引用，应用/执行再验registry |
| connections.compatibility_profile_revision | INTEGER NULL或正整数 | 必须与profile ID共同存在并匹配协议key |
| connections.provider_preset_id / provider_preset_revision | TEXT NULL或稳定ID / INTEGER NULL或正整数，二者成组 | 可选来源；不把显示名当身份，不据最新preset改写已应用配置 |
| config_revisions.codec_version | INTEGER正整数；旧行识别固定v1，新配置使用受审新版本 | 选择规范编码与seal校验，不能serde补字段后重算旧hash |

旧配置新字段只允许成组缺失；部分NULL、0、负数、空ID、错误类型、越界或未注册版本不得降级为legacy/latest。protocol ID未知的历史仍可读为不可执行，不抹掉原ID为unsupported。新记录的精确绑定加入配置语义、RequestPlan、测试和缓存身份；源码/目录变动不能重解释历史。

### 14.2 用户预设的不可变版本

候选表`provider_preset_versions`位于当前profile数据库，复合主键为`(preset_id TEXT,preset_revision INTEGER>0)`；记录namespace、provider_id、display_name、完整protocol/profile key、非秘密endpoint/部署/认证种类、受控options JSON、规范内容hash、创建时间与来源。id不得NUL，用户只能写user.*，builtin.*只读；options须JSON object且过profile schema，不能存秘密、任意headers或vault引用。

新revision插入而不原地更新；相同主键/不同内容拒绝。合法新导入生成草稿Connection；已应用配置已复制冻结的有效值，不能跟随文件或latest指针变化。托管文件保存导入来源，数据库不可变内容是权威；损坏文件只影响该导入。跨档/设备复制为待配置对象，不自动取得凭据权限。

Connection可保留同库来源关系，但运行授权不得依赖可删除的预设文件或卸载后的执行目录。无适配器的旧预设和历史版本可显示不可执行；新应用/发送严格验精确注册。实体物理删除、配置GC和来源归档规则在新迁移/Repository测试中冻结，不绕过原历史保留约束。

### 14.3 typed凭据与OAuth候选关系

| 候选对象 / 字段 | 类型与边界 | 约束 |
|---|---|---|
| credential_refs.auth_kind（扩展既有） | 受审TEXT种类 | 当前仅api_key；新增百度/有道多字段、Google服务账号/用户OAuth类型，不含TMT/TC3 |
| credential_refs.envelope_schema | INTEGER NULL或正整数 | NULL仅识别旧单Key编码；新Vault envelope注明类型/版本；SQL不存secret内容 |
| connections.auth_mode（扩展既有） | 受审TEXT | 保留none/api_key旧值，新增签名/OAuth模式需与目录、凭据类型一致；不能只改UI选项 |
| oauth_grants.grant_id | 非空TEXT主键 | 新授权生成新grant；profile隔离，引用选定credential来源及授权epoch |
| oauth_grants.issuer / client_id / scope_snapshot | 非空TEXT / TEXT / 合法且规范化集合 | 来自受控授权流程；scope受adapter上限约束，不把普通字段传入任意token URL |
| oauth_grants.principal_subject / principal_state | TEXT NULL / 受审状态 | 只存可信来源主体；未知状态明确，不用显示名认作同账号 |
| oauth_grants.authorization_epoch / state | 正整数 / 受审枚举 | 撤销、换主体或scope改变推进授权隔离，阻止旧任务提交 |
| oauth_token_material_revisions.(grant_id, material_revision) | TEXT＋正INTEGER复合键 | grant外键，材料每版不可变；只存Vault引用、时间和安全来源，不存token/私钥 |
| oauth_refresh_cursors.(grant_id, material_revision, row_revision) | grant唯一引用 / 正整数 / 正整数CAS | cursor指向同grant已有材料版本；并发轮转CAS，失败不能覆写胜者或切账号 |
| request_attempts的授权lease来源（候选） | grant/epoch/material revision等无秘密引用 | 每attempt可查实际授权；任务冻结grant/epoch，token刷新不是用户换配置 |

OAuth是否需要独立授权快照表、复合FK、grant不可变部分与可变epoch的具体拆表，以及与现有credential pool/test_record_credentials的关系，须在实施DDL评审中闭包；上表是字段职责，不冒充已编译的CREATE TABLE。旧config与新grant关系必须有真实非法写入与外键负例，不能只验证JSON字段名。

access token默认短期内存缓存，不写普通SQL；refresh token/服务账号私钥和轮转版本只入系统Vault。OAuth用户授权采用系统浏览器PKCE/有界loopback，服务账号采用受审认证库。凭据类型、授权身份和秘密材料version分开，轮转不得原地改写冻结secret；撤权/超时/取消后不允许刷新后台任务继续提交。

### 14.4 新迁移、旧seal与字段验收

迁移用新编号，不修改既有001/v2 SQL或刷新旧migration hash。新增字段缺省如何映射旧codec需先有冻结旧库样本；旧revision按原规范编码验证原seal。新配置明确新codec/绑定字段，不回写旧版本为新形状。迁移事务失败须回滚；旧客户端遇到新schema明确拒绝，不半读半写。

独立验证至少覆盖：字段类型/NULL组/正数/超大整数/ID与Unicode/NUL；协议/profile跨版本关系；重复预设与命名空间覆盖；未知协议保存历史但应用拒绝；新旧codec/seal保真；grant/cursor跨主体跨scope与悬空复合FK；刷新轮转CAS/崩溃/撤权；删除/GC保留引用；磁盘满与并发回滚。同步更新新schema-contract、字段字典/边界、迁移fixture、PRE-14支持矩阵和产品存储adapter后，才可宣称本节存储验收通过。

本轮只更新解释文档并登记[设计工作项](../../openspec/changes/specify-full-provider-registry-design/proposal.md)，不运行schema写出或修改旧字段期望。现有守卫/数据库通过不继承给本节候选字段。


<a id="offline-translation-db-candidate"></a>

## 15. 本机非大模型翻译：候选字段与版本（2026-10-30）

依据为[模型配置PRD第7章](../product/model-configuration-prd.md#offline-translation-prd)、[技术主稿第5.13～5.18节](japanese-study-and-llm.md#offline-translation-backends)、[三backend/四引擎机器目录](model-service-registry-design.json)及[本轮设计工作项](../../openspec/changes/design-local-offline-translation/proposal.md)。以下是独立候选，不是CREATE TABLE、生产DTO或已批准迁移。本轮13HTTP协议/21预设、原表/字段守卫、凭据与历史结果均保留；新结构验收全部not_run。

### 15.1 独立OfflineEngineConfig，保留HTTP原关系

HTTP继续由connections/models及AdapterBinding表达。离线配置没有必填URL、LLM模型ID或API Key，因此不将engineId写进connections.protocol，不新建虚假models，不把DLL路径装进networkMetadata。

拟新增offline_engine_configs作为配置revision中的独立资源；翻译用途有序引擎项改用EngineTarget带标签引用既有model_config或offline_engine_config。以下字段均为候选名，实际新SQL与同版本复合FK须另审：

| 对象 / 候选字段 | 类型与边界 | 关系与用途 |
|---|---|---|
| offline_engine_configs.(revision_id, engine_config_id) | 非空TEXT复合主键；ID格式/NUL/大小按新合同冻结 | 同一配置revision中的稳定实体；隶属config_revisions，不原地修改封存版本 |
| engine_id / engine_contract_revision | 受限非空TEXT / INTEGER正整数，必须成组 | 精确OfflineEngineRegistry key；未知版本历史可读但不可运行，不回退latest |
| execution_backend / display_name | platform_native或local_worker / 有界TEXT | backend必须匹配该engine合同；显示名不进入请求身份 |
| source_language / target_language | 非空规范语言标识 | 明确语言/简繁；应用及派发核对实际语言对，不把unknown算supported |
| route_policy / route_snapshot_id | direct_only或explicit_pivot / 非空TEXT引用 | 直达也有明确route；中转冻结有序逐跳语言和资源，不依赖自动pivot |
| resource_snapshot_id | 非空TEXT引用；同profile已有不可变快照 | 应用配置必须固定完整资源身份，未完成草稿用专门草稿可空规则，不伪造就绪快照 |
| installation_id（适用时） | 非空TEXT或仅系统backend可不适用 | 本机定位/执行环境引用；不作为资源内容身份，不跨profile/机器自动授权 |
| options_schema / options_json | INTEGER正整数 / 合法JSON object | 仅受审engine参数，不含脚本/任意headers/秘密，关键关系不只藏JSON |
| binding_engines.target_kind（扩展候选） | model_config或offline_engine_config | 与原模型引用、新offline引用构成严格XOR；不能两边都空或同时存在 |
| binding_engines的offline目标引用（候选） | revision_id＋engine_config_id复合引用 | 引用同revision配置；仅translation允许；grammar/context_word由SQL可表达约束与Rust双验 |
| config_revisions.codec_version（第14章候选延续） | INTEGER正整数；版本有受审映射 | 区分原codec、HTTP注册扩展和offline union；本轮不自行分配生产codec号 |

现有binding_engines模型引用的NOT NULL/外键不能在未审迁移中直接放宽。必须同时证明：每项恰有一个目标、同版本/同profile关系、不悬空、不跨用途、传统/离线不能学习、旧模型引用保真。是否采用双可空复合FK或独立目标表、怎样实现XOR和复合关系，须在新DDL评审中选定唯一方案；本表不把逻辑字段名当作已成立SQL约束。

草稿允许缺字段，与可执行快照分开。保存不完整草稿不得伪造resource_snapshot；新应用版本必须完整验证目标/资源/route，并通过既有seal与CAS原子切换。应用一个离线实体只合入明确依赖闭包，不顺便应用HTTP或学习草稿。用途、配置/安装停用闸和取消epoch继续由原Rust业务规则管理，不另造一套离线授权逻辑。

### 15.2 资源快照、安装定位与观察代次分开

| 候选对象 | 主要字段与类型 | 不可变/可变边界 |
|---|---|---|
| offline_installations | installation_id TEXT主键、machine/profile归属、engine key、受控本机位置、Windows/Bottle执行环境、row_revision INTEGER正整数 | 本机定位信息按CAS修改；修改不回写历史或已封存资源；不导出绝对路径/本机授权 |
| offline_resource_snapshots | resource_snapshot_id TEXT主键、engine contract、backend、平台/架构、引擎/数据/分词/分句版本、manifest hash或明确unobservable状态、规范内容hash | 创建后不可变；是历史/测试/请求资源身份，不能因安装被替换就UPDATE |
| offline_resource_snapshot_files | snapshot_id＋受控相对文件key、文件长度/摘要、角色与版本 | 同快照复合引用；拒绝目录穿越/NUL/重复key；商业文件只记合法检测结果，不入数据库/发行包 |
| offline_translation_routes及有序hops | route_snapshot_id、明确源/目标、direct/pivot、hop序号、每跳资源snapshot与语言对 | 不可变规范路线；direct恰一跳，pivot连续且与首尾语言匹配；无循环/重复/无限跳数 |
| offline_resource_observations | installation或system key、observed snapshot、observation_epoch/row_revision INTEGER正整数、可用/缺失/变化/错误、OS/API/时间 | 可变即时事实；探测失败不修改旧快照，不把“上次成功”当当前可执行 |
| test_records的offline范围（扩展候选） | probe/translation阶段、完整EngineTarget/资源/route/语义hash、实际终态/时间 | 保留旧测试；语义/资源/观察代次变化派生stale；探测通过不当翻译通过 |
| request_tasks / request_attempts来源（扩展候选） | EngineTarget、resource/route、worker build/协议/instance与实际平台来源 | task固定配置语义；attempt记录实际执行身份，复核不符则拒绝提交；不写DLL内容/全量原文日志 |

所有关键关系需外键/唯一性/正数/NULL组及Rust校验共同约束，不能只加无约束JSON。快照header/hash/子行封存与配置一样须防后插入改写；GC要等待head、测试、历史任务/结果、缓存与运行lease等引用解除，保留身份/回收事实，不能通过删除目录或snapshot行绕过历史归属。原缓存清理政策不因新增FK变成永久保留所有结果。

本机安装可读不等于用户已获自动化/分发许可；SQL只记录来源/许可审阅状态与真实身份，不把字段boolean当法律证明。许可材料与实际文件另按合法来源管理，导出配置不携商业DLL/词库、语言包或系统资源副本。

Apple不能取得精确系统语言包版本/hash时，不填假版本或固定空hash：快照明确version_observability=system_unavailable，并保留OS build、策略/API、语言对及当次observation epoch。无法证明资源跨重启/更新相同，测试与缓存不跨该观察代次复用。系统installed/supported/unsupported是观察事实，不用一个“已安装”位替代可运行、许可或样句质量。

### 15.3 请求、测试、缓存与取消

新canonical请求身份纳入backend、engine key、engine config revision、完整资源snapshot、worker build/协议（适用）、逐跳route、语言方向和实际处理规则，继续包含原句版本/上下文/术语/配置与授权身份。安装显示名/路径字符串不是唯一身份；同名文件被替换必须失效。资源变更产生新snapshot和候选配置，不能原地更新旧配置以复用历史通过。

本机probe可以仅写探测记录；真实测试必须经正式gateway/系统adapter/worker，结果只写test_records，不能写正文。测试选择真实台词仍固定原句并取得明确输入范围，取消测试不影响游戏会话。无API Key的引擎不生成credential_ref或假pool成员。

取消/用途停用/切档/应用有效新资源先在原授权事务推进终态/epoch并使旧任务失效，再触发系统取消或worker回收。迟到结果提交重新核对task/attempt、config、资源、route、profile generation与worker instance；失败/取消不得INSERT成功结果或cache。仅新句推进且旧任务有效可写原历史，不能重归属当前句。cache命中仍检查当前启用/权限/资源有效性，不能因已有缓存绕过停用。

### 15.4 codec、旧seal与真实迁移

第14章的HTTP注册字段本身尚为候选，不能假称它们已经迁移后再叠offline。实施时先冻结真实起点schema/codec、支持跨度及中间版本，再为实际一条或多条升级路线分配新编号。禁止修改现行001/v2 SQL、旧migration hash或schema-contract来制造通过。

旧revision严格按原codec验证原seal。只有已识别旧格式允许固定映射为HTTP执行；不把原数据库缺offline字段当损坏，也不把损坏/未知backend/未知codec降级成旧HTTP。新配置显式带EngineTarget与新codec。迁移不得向旧封存revision补写offline默认行后重算hash；如需新形状，创建新revision、保留来源及原历史引用，经确认后应用。

新schema旧客户端明确拒绝，不半读半写。正式迁移在一致备份副本上执行并保留源/目标schema、codec、DDL/程序/资源hash、数据转换与每路线真实断言；事务失败回滚、崩溃恢复、只读/磁盘满、外键/封存/XOR/GC负例和新旧历史显示必须实跑。导入/跨机迁移不继承本机路径、DLL/包授权或测试通过，重新定位后再检查/应用。

### 15.5 未批准项与验收映射

OT-12/13/14分别负责资源/缓存身份、codec/引用/真实迁移和可移植导入；OT-04/10/15覆盖head/CAS、终态迟到与多引擎历史。未来真实验证需同步新DDL、独立schema-contract、逐字段/每CHECK边界、字段字典、PRE-14矩阵和真实Rust存储adapter；原存储守卫通过不能证明本章已实现。

目前尚未批准：EngineTarget最终SQL方案、resource/installation/observation具体拆表、Apple不可观察资源缓存作用域、新codec号/升级跨度、容量/路径政策与旧数据转换。它们属于独立字段与契约审阅；用户本轮同意补设计不提升字段或正式发布状态。本轮不运行迁移，不改用户数据库，不重写原计划CSV，后续执行为not_run。
