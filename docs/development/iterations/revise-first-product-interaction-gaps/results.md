# 六项PRD与交互设计修订：实际结果

## 交付

- PRD：reading-window-session-prd.md，34条RW候选验收，全部not_run；FP表及生成OpenSpec目标同步，原97/153计划身份保留。
- [本轮设计入口](../../../design/first-product-v1/interaction-review.html)：预览、HUD拖动/快捷键、专用Select、全量会话、分词/注音/材质、组件状态。用户追加的常用操作已前置HUD且more去重。
- 原型QA：29/29、35张截图，0console/HTTP，16输入文件前后hash相同；[实际回执](../../../../.build/design-six-gaps-20261019/qa/2026-10-19T14-23-19-999Z/verification.json)。
- 既有旅程：17组、50图，0console/HTTP，输入前后hash相同；[实际回执](../../../../.build/design-six-gaps-20261019/legacy-qa-attempt-04/verification.json)。
- [独立审阅](../../../../.build/design-six-gaps-20261019/context/independent-review-final.json)：12项审查通过，636保护文件零漂移；不等于用户批准或真实产品验收。
- [离线候选包](../../../../reports/luna-first-product-design-six-gaps-20261019.zip)：96,048,313字节，SHA256 17386baa139d091d7af6afa42a88a9f548013e5a7bbadc6df599036159f69d79。

## 工程回执与待评审

已执行 `npm run verify -- --change revise-first-product-interaction-gaps`。首轮实际回执 `.build/quality/20261019T141521.592935Z.json` 因 `baseline-integrity` 和 `technology-and-design-contract` 拒绝：PRD、整稿、style-guide是本轮待审候选，原R16及design_sources锁仍保留原值。其余范围、规范映射、五类负例、技术结构/模块/目录和实际TS/Rust编译已执行；不能用刷新指纹把未评审稿变绿。

最终冻结交付后再次执行同一命令，实际机器结果写入[最终工程摘要](../../../../.build/design-six-gaps-20261019/engineering-final-summary.json)。本文件不预填最终门禁通过，不把设计QA/独审提升为产品通过。

## 为什么此前没有报错

完整证据和后续可执行验收见[漏检原因](../../design-review-gap-analysis.md)。核心是有限状态目录、受控tokens替身和缺少真实平台动作断言；不是已捕获差异被像素算法漏掉。本轮独审再次发现只比数量不足，已补空Select键盘、跟随序列/scrollTop和retry清错负例。所有旧失败与旧通过原样保留。

## 仍未发生的事项

未修改apps/crates/native、原PNG/token/catalogue、License或R16；未重建正式包、采新游戏帧、注册真实全局键或跑真实词法/游戏/模型。未提交或推送。候选功能进入产品实施前仍需本轮评审、精确契约/资源/权限与必要schema补充及真实运行adapter。

## Select间距后续修订

用户报告高亮重叠；实际前像为0px间距，选中行is-active与鼠标hover同时填色。只修候选controls.css/js：2px行隙、不可压缩行高、统一活动态、禁用项无hover填色；不改token/产品代码/原图。新增真实pointer/keyboard/长列表几何断言。新设计QA31/31、37图，既有17组/50图通过；独立20项检查通过。具体回执与交付见 `.build/select-spacing-20261019/`，旧回执和旧包保留为历史，不沿用为新源通过。
