# 最新设计到客户端、PRD、测试与准出的同步计划

- 用户授权：以最新设计规划当前客户端文档改动和PRD变动，完成后更新测试用例与准出判断。本轮仅规划/文档/工程门禁，不改客户端、运行包、DDL或新UI。
- 基线：FP1-20261030-R74，SHA f9e561d0f1b78f4414f519d098693172fd69d3a940f5d2dfeb8c4dc377d45331，Main2087源完整冻结。
- 目标稿：first-product-v1/candidates/guided-service-reading-20261030，布局/配置目标pending_review，不提升用户确认或产品状态。
- 产品追溯：FP04/06/08/09/11/12，AI-CFG01～16、RW34，PA12/OT16/SS12/FL10/GD12与新增独立登记GUI01～08缺陷回归。原128/153计划及97来源、111CSV冻结保留。
- 工作项：客户端现状与目标差距/模块职责/依赖/存储契约影响；PRD当前规则与迁移边界；逐项逐层用例/前置/Owner/正式包证据；新增准出闭包机器校验和真实release路径拒缺项。
- Owner：client_alignment_audit五架构/客户端docs；readiness_test_audit最新coverage计划/工具与测试docs；Root PRD/总计划/verify最小接线与采用；current_alignment_governance OpenSpec/context/独立审阅。作者不能自审，互不回退编辑。
- 模块：M03阅读、M04配置、M05翻译、M06服务、M10历史/三数据集、M11偏好、M13UI、M15诊断准出、M16凭据；按权威模块表，不沿用旧误映射。Rust权威/窄Tauri/独立Core与worker保持。
- 允许路径：精确docs、新current_product_coverage.py/test文件、verify注册/真实release调用、M15唯一hash及CSV派生、本change及R75评审基线。禁止旧计划CSV/JSON/候选图/产品/技术锁/活跃阶段变更；FP表若不改则不生成spec。
- 前置：R74完整map/原字节、最新UI manifest/catalog/当前真实源码只读证据、原准出工具。新真实层缺adapter/凭据/资源/桌面/同包实测保持not_run或blocked，不造成功。
- 验证：新登记schema/完整ID/25服务13协议4引擎/GUI8映射/每层variant和准出反例；旧68工程检查及新增checks、五负例/实际TS Rust编译；真实release负例精确拒绝新范围缺项；HTML报告渲染/链接QA。
- 完成条件：最新文档互链/明确现状与待实现任务、测试及准出可由程序校验，实际回执/源码identity与独立审阅绑定；本轮规划完成不等于产品可发布。
