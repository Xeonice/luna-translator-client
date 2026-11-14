# 迭代卡

- 需求：FP-06/08/10/11/12，AI-CFG01～16保留，OT01～16新离线验收，OTD001～004本轮文档/设计检查。
- 设计：first-product-v1/candidates/local-offline-20261030/index.html?scenario=settings；pending评审，旧批准原字节保留。
- 模块/组件：M04配置、M05翻译、M06适配、M10存储、M12平台、M13UI；M16只沿用现有凭据隔离；C04自绘控件、C26配置编辑。原矩阵不增改产品实现状态。
- Owner：docs worker仅5MD/JSON；UI worker仅新候选目录+整稿README；Root负责追踪、测试、PRD索引、工具、独审与精确采用；context独立只读治理。所有人不覆盖他人。
- 允许路径：verification.json精确列举，不授权apps/crates/native、原approvedPNG/techlock/approval/历史CSV。
- 依赖：现R70快照、既有整稿/共享资源、官方系统API/Argos/原版DLL源码资料。真实运行依赖各平台系统/语言包/授权安装/位数/网络断开及最终签名包，见新测试计划。
- 验证：新静态守卫、真实Playwright候选旅程与旧截图参照；原32守卫/五负例/TS和Rust编译；npm run verify -- --change design-local-offline-translation。结果另建run，不预填。
- 完成：PRD/候选整稿/架构/测试/HTML均更新，独审、真实设计测试及完整工程门禁通过。产品实测和字段批准不由此通过。
