# 必要配置与阅读优先：迭代卡

- 授权：用户指出当前会话样式问题，历史及日志应在后方展开；翻译配置只让用户配置其需要且能理解的内容。本轮仅PRD/技术/HTML设计/检查，不改客户端、DDL或正式包。
- 基线：FP1-20261030-R73，SHA4bac316bd1da9da25e2e3657856b142ed6fddd9e96dc9cf3f34dadcfc8e5dc92；完整源快照已冻结。
- 需求：FP04/06/08/09/11/12，AI-CFG01～16相关条件配置，RW17～23与阅读预览。新增GD01～12计划not_run，旧PA/OT/SS/FL和原CSV不改。
- 设计：first-product-v1/candidates/guided-service-reading-20261030，pending_review；阅读首屏独立当前句、辅助预览、后方历史/全部提取/诊断默认折叠；服务先选具体供应商/引擎/接口，必要字段主表单，高级按需，精读/查词独立可选。
- 模块/组件：M04配置/M05翻译/M10历史/M11诊断/M13UI，复用C04/C26、已有saved-service picker/配置版本/列表查询；Rust业务权威、窄Tauri及协议身份不变。
- Owner：UI flat_design_audit新候选；docs flat_docs_audit九文档与呈现目录；Root新工具/验证；flat_governance治理。非作者独立代码/文档/packet审阅，互不回退编辑。
- 允许路径：本change、准确新候选、十文档（含服务呈现目录）、新guided工具、verify最小注册、M15唯一证据hash派生；如FP表变则仅first-product spec确定性派生。禁止旧源/图/token/协议数组/DDL/产品改动。
- 依赖：R73原始字节与完整map、Chrome/Playwright；字段展示目录需精确绑定21preset/13protocol/4offline，缺真实默认值不编造，演示数据不代表实际探测。
- 状态：配置草稿/测试/保存不切服务，选用后续句/范围/启停独立；折叠不删除全集、不停订阅、不重建预览。
- 验证：实际Playwright正常/空/失败/取消/字段必填/品牌接口/高级/键盘/窄窗/样式截图、正负静态守卫、npm run verify -- --change design-guided-service-and-reading，准确证据hash与独立审阅。
- 完成：真实源匹配的检查通过、新设计可review；用户确认/正式客户端/服务API/系统与游戏仍独立待验。
