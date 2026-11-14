# 当前产品与研发文档

当前工程任务为[公开 GitHub 与远端门禁激活](../openspec/changes/activate-public-github-gates/proposal.md)，仓库为 [Xeonice/luna-translator-client](https://github.com/Xeonice/luna-translator-client)。它与原版项目 fork 分开维护。九类判定器和真实 Codex hooks 的上轮本地验证已完成；本轮继续实际验证冷 checkout、固定资源/真实证据、正常及拒绝 PR 与 main 必需检查。当前进度见[GitHub 激活说明](development/github-activation.md)；实际执行结论以原件为准，不以公开仓库或 YAML 存在宣称 CI 通过。

产品源仍按R93登记，当前产品验收 **incomplete**。门禁有效、工程检查、人工视觉确认、正式签名包与SR／MO／Never7／樱之诗真实回归分别判断；本轮不改应用、不替换安装包、不提升产品准出。前序绑定QA维护与原件状态在其OpenSpec和当前阶段记录中保留。

本页与[current-documentation.json](current-documentation.json)构成唯一现行阅读入口。原始PRD、License、已批准设计源/图、冻结用例和真实执行原件保留；[历史归档](archive/documentation-r75/index.md)只供追溯，不作为另一份当前规范。

## Review顺序

| 要核对什么 | 现行文档 |
|---|---|
| 本次任务、先后依赖与页级完成条件 | [完整绑定旅程计划](development/binding-journey-plan.json)、[整页交付计划](development/current-client-alignment-plan.md)、[迭代执行规则](development/iteration-governance.md)、[迭代卡模板](development/iteration-template.md) |
| 产品应提供什么 | [PRD索引](product/README.md)、[首版范围](product/first-product-scope.md)、[游戏入库与关联](product/game-library-window-prd.md)、[AI阅读与学习](product/ai-reading-prd.md) |
| 阅读与配置的用户流程 | [阅读/窗口/会话PRD](product/reading-window-session-prd.md)、[模型与翻译服务PRD](product/model-configuration-prd.md)、[历史筛选与分页PRD](product/history-session-paging-prd.md) |
| 已确认设计目标 | [完整设计入口](design/first-product-v1/README.md)、[完整源版批准记录](development/design-approval-complete-20261031.json)、[全页与状态清单](development/all-page-design-coverage.md) |
| 客户端如何落实 | [研发基线](architecture/first-product-engineering-baseline.md)、[阅读技术设计](architecture/reading-window-session-technical-design.md)、[语言与服务技术设计](architecture/japanese-study-and-llm.md)、[目录/模块职责](architecture/engineering-layout.md) |
| 当前代码与运行能力 | [客户端运行说明](development/desktop-client.md)、[当前源审登记](architecture/implementation-status.json)已采用R93的78条职责／739个产品源；原登记、受控测试、正式包和运行验收各有独立身份 |
| 怎样验证、怎样放行 | [逐层测试规划](development/first-product-test-plan.md)、[当前用例闭包](development/current-product-test-coverage.md)、[设计对齐机制](development/design-alignment-and-release.md)、[工程门禁](development/engineering-gates.md)、[产品完成判定](development/product-acceptance-loop.md) |
| 门禁设计与执行依据（已授权实施） | [门禁正反例与触发设计 · HTML](development/gate-validation-design.html)：九类门禁（含视觉人工确认）、Codex hooks、32组正反例；实际执行见统一结果页 |
| 当前工作身份 | [当前阶段](development/current-phase.json)、[基线指纹](development/iteration-baseline.json) |

## 当前设计、实现与验收状态

- 设计目标ID为`guided-service-reading-20261030`，完整组合为`complete-guided-history-20261031`，源版状态 **approved_by_user**。guided57源与R79历史4源共61源由[完整批准记录](development/design-approval-complete-20261031.json)绑定；原HTML中的pending_review是冻结捕获标签，不能覆盖独立批准事实。
- C35另采用[五项预览组件补充](development/design-approval-preview-20261104.json)，5文件批准不替代其他整页，也不自动形成完整Main参照。历史始终保存、改为状态与入口的语义由[用户决定](development/settings-history-policy-20261104.json)约束。
- R90源码已有独立当前卡/后置折叠区、真实词法/词卡、同源服务目录与字段投影、引导编辑器、版本化服务保存/选用、共享浅层选择器及13协议wire。保存服务测试与Google交互已接实际受控链，Google并发/兼容修复已通过限定独审；离线最终资格、模型发现客户端流程、回退闭包与完整页面/正式包仍有缺口；受控测试和局部源码审阅不能表示产品通过。
- 原4态参照与执行原件保留；通用设置采用已确认的V3与三类状态补充，另以14状态/27固定采集组合建立独立公开WK与同签名包Native两层执行。参照审核见[current-general-reference-review.json](development/current-general-reference-review.json)，最终候选的实际执行状态见[本轮机器结果](../.build/general-visual-closure-20261108/phase-2/current-result.json)；源确认、参照、真实组件和正式包分别判定，不能代表当前944项适用状态或四游戏。用户已接受General S5当前微小像素差异并要求自动门禁消费，精确[决定与原件绑定](development/general-native-visual-acceptance-20261109.json)由独立裁决check处理，保留旧RGBA失败而不再追平；接受仅绑定既有S5精确包与Source62，不能自动授予服务列表修复的新包。实际执行见本轮机器结果，不能据规范或局部接受宣称全产品通过。原802项及此前7项义务全部保留；本次实际AST补充89项适用状态与3项旧入口消失检查，原901项继续保留，当前AST另加43项，共944项。未保存确认与Google撤销仍有设计缺口，完整复制仍阻断。
- 当前产品准出 **blocked**，完成判断为 **incomplete**，`next_stage_allowed=false`。最近完整工程[123步通过](../.build/quality/20261109T082542.574702Z.json)仅绑定R91/a005，1837项执行计数及原失败保留。当前[绑定变更](../openspec/changes/align-approved-binding-journey/iteration-card.md)已完成r3计划/门禁审核及真实续写准入，UI受控40场景与Native源码/265项受控测试已有各自运行证据；这些不同源码切面的局部结果不能拼成最终同包验收。R93真实采用及父阶段同树检查已完成；[后继QA计划](../openspec/changes/register-binding-impact-surface-mappings/iteration-card.md)修正全局接线和历史消费者，严格schema5须重新执行。C13～15独立参照、实际Native writer、正式包和同包SR／MO／Never7／樱之诗及真实翻译仍未闭合。S5原现场与历史翻译结果保留，不继承到新包。

原153执行组合、RW逐层、存储专项、当前70来源/576观察、独立4项Windows资格与全部已确认设计义务累计维护。计划review、文档检查、源码审查、真实组件、正式包和发布分别作结论。
