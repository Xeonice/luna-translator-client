# R28：带姓名前缀的引号片段增长修复

日期：2026-10-20。授权：用户持续要求定位修正并完成同包四游戏回归。对应既有FP-04/09/12、RW-20/23/29；当前07已完成61/61工程门禁，但SR第8屏原文重复，产品回归失败，不能按工程绿关闭。

## 证据与范围

07/source bb1939...，SID1792500400593-59092-0，TextOutW selection revision2。raw179为“【法珞】「这么说来，”，2102ms后raw186为“「这么说来，库里斯同”，399ms后raw187为“学是三年级？」”。V2仅认识整段前缀增长，将带姓名的旧片段与省略姓名的新增长片段全部Append，形成重复。原始回调与source_refs完整；模型恰好修正译文不能掩盖原文错误。所有旧07包/原图/失败与工程绿回执保留。

设计定位：现行整稿Main/HUD当前原文与会话历史，不改视觉、交互、token或批准图。模块M03/M10/M05/M15；组件C14/C16/C17/C18/C34/C36。独立Core/ABI1/JSONL2、UI/DTO/DDL/依赖和模型权限不改。

## 编码前候选政策（以独立架构审阅为前置）

具名V3，保留V1/V2历史政策。只对可证明的结构前缀增长增加一种fold：当前未完成文本内，支持且正确嵌套的未闭合引号起点之后的整个后缀，是incoming的严格前缀，incoming包含新增字节，才允许保留引号前的前缀并替换该后缀。不进行任意最长后缀/前缀重叠去重；不删除合法完整重复、不越过回退/相同事件/硬限/选流/暂停/断开或已finalized边界。不以游戏名、姓名或本句字面判断。

纯领域计算UTF8 prefix字节位置；仓储按现有source_refs精确重构保留prefix（可能截在旧某个ref的合法UTF8字符边界），添加incoming真实来源，不改旧revision或原始payload。新processing revision和snapshot/cache身份记录此规则，生产ingest/完成/timer统一V3。6秒、64KiB、128refs/revisions与finalized+latest模型任务资格保留。歧义与未覆盖情况如实记录，不声明可靠页信号。

## Owner与允许路径

- 实现Owner capture_continuation_impl：crates/app-core/src/domain/capture.rs；crates/app-infra/src/storage/queries/capture.rs；crates/app-infra/tests/storage_product.rs；crates/app-infra/tests/application_runtime.rs；必要身份预期crates/app-infra/tests/storage_acceptance_support/fields.rs。
- root：本卡、scope/spec/design/tasks/proposal集成、必要精确status和R27字节归档/独审后R28身份；同源候选08打包/实机。独立architect/code-reviewer审差异，governance worker候选不能自批。
- 不改storage pins对应的database.rs、schema/DTO/Core/资源/技术锁/旧债务/批准截图。不提交推送。并行协作者不得回退或覆盖他人修改。

## 先红后绿与完成条件

真实SQLite入口先复现姓名+未闭合引号后缀严格增长（使用合成日文/中文/Unicode，不只实际SR一句），证明旧生产错误并保留红测。V1/V2旧断言不弱化；新V3域/SQL/真实timer证明修订旧版不变、refs UTF8精确重构、事件幂等、合法完整重复/相同尾片段/非增长/回退/错配引号/不平衡分隔符/硬限/旧策略隔离、缓存身份和open零网络/完成精确一次。若既有测试前提或字段期望必须迁移，逐项记录，不改预期迁就错误。

独审六类边界及实际diff，精确采用R28后冻结源。新候选08构建/签名/seal/EXE/ZIP及完整统一verify；不得继承07的61绿。实机先SR至少五屏含慢分行与本类有姓名增长，对照Game/Main/HUD全文、rawrefs、译文任务；SR通过才以同08包继续MO/Never7/Sakura，排除Ever17。无执行not_run、缺依赖blocked、失败保留。

状态：编码前计划记录；本卡不预填执行通过，后续运行独立记录。
