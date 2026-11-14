# SR-DESIGN迭代卡

日期2026-10-27，父基线FP1-20261027-R55。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-03, FP-05, FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M12；组件C35。UI负责canonical设计/设计验证10文件；Root负责approval record、本change及authority；AP/UI独审。不可覆盖/回退他人。。并行协作者不得覆盖或回退他人。

产品精确路径：
- docs/design/first-product-v1/review-gaps.js
- docs/design/first-product-v1/review-gaps.css
- docs/design/first-product-v1/index.html
- docs/design/first-product-v1/data.js
- docs/design/first-product-v1/README.md
- docs/design/first-product-v1/style-guide.md
- docs/design/first-product-v1/integration.md
- docs/design/first-product-v1/coverage.csv
- docs/design/first-product-v1/manifest.json
- scripts/report/verify_interaction_gap_design.cjs
- docs/development/design-approval-20261027-compact-preview.json

依赖：R55代码/门禁已结束65/66仅旧visual失败；最终验证-only计划已建但未构建/运行。用户随后明确批准紧凑候选，仅授权设计更新，不是产品实现或游戏通过。最终运行源需本片完成后显式重新冻结/核对，原计划前像/审阅保留。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。

类型边界：本片仅为设计原型与其验证工具整合，不含产品application源码，因此登记为tooling/design-only；必须实际执行interaction-design-execution，不以此宣称product-ui-capture-v4、签名包或游戏通过。旧R55 product视觉失败继续保留。

Root技术配套：当前设计获批更新后，原v4历史来源的data/index/coverage/integration/manifest/style-guide共6项identity路径内容变化。仅将physical_path改指已有design-origin原同SHA文件；逻辑路径、原SHA、306闭域、原PNG/算法/阈值均不变，不刷新旧expected哈希或采用新图。Root拥有此额外map路径并单独审阅。

独审新增配套P1：Python/Node历史来源许可仍硬限定原5项。仅扩充6条明确same-original-SHA design-origin许可并补精确正负例，禁止通用fallback；因此增加两parser和两守卫测试精确路径。本片含测试基础设施apps/tests源码，按product-kind机器范围登记，仍不含应用实现。登记并实际重跑product-ui-capture-v4，核对历史来源可解析且原图、阈值不变；其既有产品差异不得由本次设计确认自动解除。此前tooling/design-only说明由此范围补充取代，产品验收仍not_run。

独审第二项P1：选择当前同一目标后取消，会话预览详情错误借用共享采集Map中的候选归属。修复只在上述review-gaps.js与设计交互检查中完成：采集事实继续共享，详情归属从真实触发宿主确定；补同目标候选取消后会话详情/复制与候选详情的实际旅程。原失败审阅及截图保留，修复后的两文件、manifest源指纹及完整回执重新冻结并另行独审，不沿用修复前通过记录。
