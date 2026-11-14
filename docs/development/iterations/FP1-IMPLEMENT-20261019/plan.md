# 首版实施迭代卡

- iteration_id: FP1-IMPLEMENT-20261019；baseline_id: FP1-20261019-R6。
- 用户授权：2026-10-19本轮原文“开始动工，按照事先规定的架构和设计进行开发，完成后进行对应的测试”；包含FP-01..12产品编码、打包与对应测试，不包含提交/推送。
- 源码身份：无Git提交的来源快照见本change/source-snapshot.json。
- 逐项需求/设计/模块/组件/Owner/允许路径见traceability.csv和verification.json；状态不预填通过。
- 原始PRD、License、设计源和已确认图不变。旧探针业务文件在新层接入后删除，保留可复用Hook边界。
- 前置：新Core仓库commit e9242ebac32a09e911f306b7c90714891c217302可用；MeCab/UniDic/中文词典、真实模型服务、游戏进程、正式包与视觉adapter待实施或核验。
- 视觉：当前first-product-v1全稿为实施目标，材质确认范围沿用R6；候选截图独立输出，整稿评审状态不擅自提升。
- 数据：仅首版新配置档schema1；生产迁移新建，历史JSON不静默覆盖；未知数据库版本拒绝并保留原件。支持路线与测试结果单独记录。
- 任务分工按模块明确，Root集成/门禁/独立审阅；协作者不得覆盖他人变更。
- 验证：各模块实际单元/集成、全workspace编译、五类负例、Playwright候选与设计对照、原生正式包/实际Core和游戏回归；依赖不足列blocked。
- 完成条件：全部声明的检查有真实回执；最终报告列包/资源身份、passed/failed/blocked/not_run及限制，不用门禁自测代替产品验收。

## IMP-MODEL-RECOVERY-20261019

用户授权继续收尾，依据 R12 和 `openspec/changes/implement-first-product/model-test-recovery-work-item.md` 修复实机暴露的配置测试失败刷新及 SSE 超时分类。复用 FP-06/08/12、现有端口/DTO/记录与 UI-05，不改变 PRD、视觉批准状态、基线或测试预期。Rust/React 回归、五类规范负例、重新打包与新包实机分别留证；8cf6 原包回归不继承为新包结果。当前 in_progress，未提交推送。
