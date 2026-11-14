# 平铺配置与当前会话：迭代卡

- 用户要求：两处Tab改同页平铺，审查无用户价值呈现。仅PRD、技术合同、完整HTML候选与设计层验证，不改产品、DDL、正式包或历史确认。
- 基线：FP1-20261030-R72，指纹2ae180cdb2d3b04dce306e098aaa7a860c021d7fc13b2224f134347c865d5b85；当前Main完整source-snapshot冻结。
- 产品映射：FP-06/09/11/12，AI-CFG-01～05/11～13/16，RW-01/17～23/31～34；新FL-01～10规划not_run，不改旧CSV/SS/OT/PA。
- 设计：first-product-v1/candidates/flat-service-session-20261030；settings服务管理与session同页区块，正常/空/禁用/高级/错误/键盘/窄窗；pending_review，既有材质与原批准源PNG保留。
- 模块：M04配置、M05翻译、M10历史、M11诊断、M13UI；复用C04/C26，目录与数据权威不改变。
- Owner：flat_design_audit新HTML候选；flat_docs_audit文档及FL计划；Root新检查器/验证/精确采用；flat_governance治理文件及事务；独立审阅由非对应作者承担。各Owner不覆盖他人。
- 允许路径：本change、上述新候选、准确相关文档、新quality/报告检查器、verify注册及M15证据哈希派生；禁止批准源/PNG/token/DDL/产品与旧候选测试。
- 前置：R72原始字节和完整源快照、已有Chrome和固定Playwright；产品仍需独立实现和正式包实测，不以设计样例代替。
- 验证：真实Playwright同页内容与条件字段、独立筛选/分页、保存不切服务、选用返回、按需高级/诊断、键盘与窄窗，源码与截图hash；正负守卫、npm run verify -- --change design-flat-service-and-session，独立代码/文档/packet复核。
- 完成条件：准确修订可交互、无页面错误、新旧检查通过、实际回执与候选身份绑定；用户新稿待审、产品not_run。
