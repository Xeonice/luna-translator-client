# 设计与职责

前置登记按规则OR/字段AND/取值ANY匹配case@variant；每项记录谁提供、Owner、门禁、交付/解除方法和未确认现状。源码维护以现有架构目录为边界：Rust单测靠近模块，真实I/O在app-infra/tests，跨模块/正式包/原生/游戏/质量在tests/integration细分；UI视觉仍唯一在apps/desktop/tests/visual。

JSON为供给/维护单一源，三CSV和HTML派生。实际运行结果和就绪复验另绑run，不把供给lint升级为passed。原始证据受控归档，report只是脱敏副本，保留期与预算待B0决定。前端报告仅增加离线过滤，不触及产品前端或Tauri。

迭代映射：FP01..12、AI-CFG01..16及原CR55/MC22/GL20不变；模块M01..16/组件C01..34不变。主责QA协调，各模块Owner实现测试。验证为20项供给守卫、导出身份、原五类工程负例/编译及报告Playwright；产品153组合仍not_run。
