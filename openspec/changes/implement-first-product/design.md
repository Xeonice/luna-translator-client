# 实施架构

## 同源多行/逐字聚合

`multiline-capture-work-item.md` 定义内部 CaptureOutcome 与 flush 端口。领域规则由 app-core 决定，SQLite writer 原子保存 raw、open revision、多 source_refs 和 finalized；原文先发布，自动模型任务只使用稳定的确切 revision。相同事件幂等，不同事件合法重复保留；前缀增长与片段追加仅限当前 session/source/selection，等待/容量均有上限。生命周期封口属于原命令事务，不在 CAS 前另行 commit。历史只投影每个 unit 的最新版，旧确切版本仍可学习。公开契约/DDL/资源/基线不改；无页边界的 GDI 聚合是有界启发式。

## 实机缺陷收尾：模型测试与流式错误

详见 `model-test-recovery-work-item.md`。Rust 测试记录提交后无论结果都发布权威投影；原测试异常优先于后续发布异常，发布故障独立诊断。UI 测试入口使用同一 finally 刷新路径，不清 dirty 或编辑基准，不自动重试保存，不放宽全局提交与草稿 head 的双 CAS。SSE transport 保留安全 AppError，timeout 不被字符串化后误报为通用中断。无新协议、依赖、字段或视觉规范变更。

内置 grammar/context_word 默认提示补充句子与名词短语、定语与中心词、所有者与动作施事、主语省略及证据引用约束；用户模板优先级和输入范围不变。提示改进不作为模型语义正确证明。既有 UniDic one-best 的上下文读音问题需要后续有界候选/不确定性契约迭代，本轮不增加词面规则。

遵循docs/architecture/first-product-engineering-baseline.md、engineering-layout.md与database-design.md。Rust app-core拥有领域/用例/投影，app-infra实现单writer SQLite、网络、系统凭据、Hook与语言端口，Tauri仅装配/角色权限/原生窗口，UI仅依赖生成DTO和application-port。

新建版本化生产迁移，不在产品运行时执行docs DDL；首个schema版本只保证新配置档，旧JSON原件保持不变，未知/未来版本拒绝。支持矩阵与每一测试结果分别冻结及记录。Core固定e9242eba、ABI1七回调，实际位数broker/Host/Hook/Injector同版本校验；选流用户显式确认。

先建立契约与数据/状态基础，再并行移植broker、实现界面与语言/模型适配。业务任务身份/取消/配置应用与历史一致性由Rust与SQL约束共同保证。回执必须来自实际编译、测试、候选截图及正式包；缺失adapter不得宣称通过。

## 模型默认模板契约 v1

`Model.defaultTemplates` 是按 translation/grammar/context_word 映射的稳定模板引用，持久化到既有 model_default_templates，不新增DDL；旧 JSON 缺省空映射。模板作用域元数据约定 v1：`Template.compatibility.modelConfigurationId` 可选，值必须是稳定 model_id；缺省为共享模板。不能使用实际模型名称、显示名猜 owner；同 owner+purpose 的模板名称不重复。`protocols` / `actualModelIds` / `sourceLanguages` / `targetLanguages` 可选显式文字列表，存在时必须精确匹配，不把未知模型推断兼容；实际请求再次验证已解析语言方向。

模板优先级：有效引擎显式模板 → 有效用途显式模板 → 游戏override场景下可识别的全局同模型引擎模板 → 全局用途模板 → 当前模型按用途默认模板 → 内置模板。模型/用途/引擎任何显式引用都必须匹配purpose和专属owner；全局同模型引擎有歧义而未明确当前引擎/用途模板时拒绝猜选。无模板是继承，不删除用户保留模板。

CopyModelConfiguration只读已保存草稿，CAS draft head及operation receipt幂等；生成新的draft snapshot，不应用。参数深复制；该模型所有专属模板独立ID、owner改为副本、默认引用重映射；共享模板/连接保留引用，并在UI明确标示。名称冲突拒绝，不静默改原模型或覆盖模板。UI逐用途默认/恢复内置与复制确认属于本波独立产品候选；批准整稿/视觉token保持原样。

兼容边界：latest-only 仅适用于本次 processing_snapshot_json 中明确 unitProjection=latest 的聚合内部修订；既有正式处理器/手动修订历史继续保持原有多修订分页语义，旧断言不变。


## 聚合实现的源码身份边界

参见 [capture-source-identity-work-item.md](capture-source-identity-work-item.md)。独立审阅后仅登记迁移执行文件的实际完整源码身份与 R13 单条指纹，不改变 SQL DDL、字段关系、批准范围、断言、阈值或产品验收状态。旧失败回执保留；新包与完整门禁重新执行。


## 两项产品验收实现

见 [acceptance-closure-work-item.md](acceptance-closure-work-item.md)。生产 Rust 单目标编译/执行输出逐项真实断言和配对数据库，独立 schema/ledger/来源复核；视觉同态参照契约需独立审阅，缺批准状态保持待决，UI/基础架构职责保持既有边界。


## 入库与目标核对对齐

见 [library-capture-visual-alignment-work-item.md](library-capture-visual-alignment-work-item.md)。恢复已有设计中的参数、批次审阅/单次原子确认及真实候选字段显示；公开命令/回执的新增影响、去重与CAS决定显式记录，不改变Core/DDL。
