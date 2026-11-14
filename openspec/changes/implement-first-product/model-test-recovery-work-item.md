# 模型测试失败恢复与流式超时收尾

## 迭代身份与授权

| 字段 | 本次约束 |
| --- | --- |
| work_item / baseline | IMP-MODEL-RECOVERY-20261019 / FP1-20261019-R12 |
| 用户授权 | 继续处理并收尾问题；修复实机发现的配置恢复与请求诊断缺陷，重新打包并验证 |
| 既有证据 | `.build/client-regression/games-8cf6f4ee-20261019/observations/model-test-cas/`、`sakura-ai-failure/`、`sakura-grammar-retry/` |
| 产品依据 | FP-06/08/12；AI-CFG-04/05/12/13；MC-03/05/06/15/17；CR-05-ERROR、CR-13-STREAM-REVISION |
| 设计状态 | `docs/design/first-product-v1/index.html#settings-window`、`#learning-window` 的失败、编辑保留与显式恢复；布局、材质、批准图及 token 不变 |
| 模块 / 组件 | M04/M06/M09/M13/M15；C22/C26/C29/C31/C33；现有 ApplicationPort、测试记录与投影，无新 DTO/DDL |
| Owner / 审阅 | root 负责 UI、OpenSpec 与实机；recovery_backend 负责 Rust；独立 code-reviewer 审阅；game_suite_context 维护上下文/报告 |
| 前置 | R12 权威文件无漂移；原 8cf6 四款五状态捕捉证据独立保留，最后 Sakura 正常断开已实机补测，须另记回执 |

## 允许修改与复用

- `apps/desktop/src/features/model-config/ModelConfigScreen.tsx`：复用一个测试执行路径，成功与失败均刷新权威快照，保留原错误、本地 dirty、草稿编辑基准；不重发命令、不自动应用或启用。
- `apps/desktop/tests/support/design-port.ts`、`apps/desktop/tests/visual/verify-ui.mjs`：现有 UI-05 扩展失败/延迟/刷新故障输入，不复制生产 CAS 业务，不删既有断言，不改批准图或差异阈值。
- `crates/app-core/src/application/runtime.rs`：测试记录提交后成功/失败都发布，原测试错误优先，发布故障独立诊断。
- `crates/app-infra/src/providers/mod.rs`：SSE 保留安全的底层错误类型，超时、网络断流、解析故障与缺失结束标记分别处理；不输出服务原始响应或认证值。
- `crates/app-infra/tests/application_runtime.rs`、`provider_protocol.rs`：真 SQLite、直接订阅、真实本地 HTTP/SSE socket 验证。
- `crates/app-core/src/application/request_plan.rs` 与 `crates/app-infra/tests/runtime_review_regressions.rs`：针对实测模型误把定语所有者当主语，增强内置学习提示的通用结构约束；先区分句子/名词短语、定语/动作施事/中心名词，省略信息明确不确定。仅默认提示生效，不改已保存用户模板、不增发上下文或词典资料。真实模型多种结构复测仍需独立判定，不能以提示文字或 JSON 合法替代质量通过。
- 本 change 的 spec/design/tasks/verification，以及本迭代 plan/traceability/verification-progress：只补追溯与真实执行状态。

范围不含分词词面补丁、替换模型、修改已保存用户模板、资源/依赖/技术锁/DDL/基线或批准截图。UniDic 消歧与模型语义质量另行记录，不为特定台词硬编码。

## 验证及完成条件

1. UI-05 保留现有保存/测试/应用分离断言，另验证失败后刷新提交版本、等待期间编辑保留、本地校验失败、刷新失败与原错误分别可见、真实草稿 head 冲突的编辑基准不被更新。不得自动 test/save/apply/enable。
2. 真 SQLite 测试失败记录、commit 增加、draft/applied head 不变；直接 watch 收到新投影，不先 snapshot 掩盖发布缺陷；刷新后保存成功，真正的全局/草稿 CAS 仍拒绝。
3. 真 SSE 响应头和 partial 已收到后停滞触发 timeout；真实断流非 timeout；正常流、截断、取消与有限重试已有回归继续执行。
4. TypeScript/测试类型、相关 Rust 集成测试、Clippy、完整 `npm run verify -- --change implement-first-product` 留实际回执。五类规范负例必须实际执行，既有视觉/存储缺口保持真实失败，不刷新基线规避。
5. 新包另记源码/可执行文件/资源/Core hash 与新 session，真实模型失败→编辑→显式保存/测试/应用无需诊断页手动刷新；四款游戏新包从 not_run 开始回归，旧包证据不改写为新包通过。
6. 默认学习提示的组装/固定输入/选词边界进行确定性回归；同一真实模型补测名词短语、显式主语句、主语省略句，原 MO 语义错误输出保留。UniDic 两个上下文读音质量问题保持未修复，不按词面补丁或固定 N-best 序号伪造消歧成功。

当前实施状态：in_progress；尚无本次修复通过声明。未获提交/推送授权。
