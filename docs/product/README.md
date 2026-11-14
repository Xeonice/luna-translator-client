# 当前 PRD

当前完整设计组合（guided与R79历史页）已由用户明确审核通过，身份见[源版批准记录](../development/design-approval-complete-20261031.json)；其他页面按[全页与状态闭包](../development/all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

首版交付目标为取文、翻译、内置分词词典、按需精读及服务配置组成的可用产品，当前产品尚未完成。下表是唯一现行PRD入口；修订直接覆盖相应规则和现状表，不在尾部追加第二份“本轮/最新”方案。历史定义与执行原件独立保留。

| 规范 | 职责 |
|---|---|
| [首版范围](first-product-scope.md) | FP-01～12及本期排除项 |
| [游戏库与窗口关联](game-library-window-prd.md) | 入库、选运行窗口/进程、一次确认关联并接入 |
| [阅读、窗口与会话](reading-window-session-prd.md) | 独立当前卡、后置折叠数据、预览、HUD、日志及词法效果 |
| [AI阅读与学习](ai-reading-prd.md) | 情境翻译、按需精读、语境查词及后续场景边界 |
| [模型与服务配置](model-configuration-prd.md) | 必要字段引导、统一浅层选用、三层资源/版本/用途、认证与恢复 |
| [历史页会话筛选与分页](history-session-paging-prd.md) | 按会话筛选、加载较早记录及对应异常状态；R79精确源已纳入完整批准 |

界面目标见[当前完整设计](../design/first-product-v1/README.md)，实施差距见[客户端对齐计划](../development/current-client-alignment-plan.md)，验收见[逐层计划](../development/first-product-test-plan.md)及[当前测试闭包](../development/current-product-test-coverage.md)。每个页面任务必须同时交付对应用户流程、完整排布和适用状态，接口前置未齐时保持未完成。目标与实际不符时登记缺陷，不能把局部控件迁移概括为整页完成，也不能把实现反写成规范或用历史截图代替新目标。

本期仅连接已运行游戏；核对窗口/进程后一次“关联并接入”启动Hook，收到候选后显式选正文。预览随有效关联自动工作；封面优先本机，无图时按授权仅保存首张有效画面。当前句独立首屏，历史、全部提取和诊断在后方默认折叠。配置按用途→品牌→明确接口→必要字段；日常选服务在浅层统一入口，未配置不可选，保存和测试不切换服务。通用设置按[已确认结构](../design/first-product-v1/candidates/general-settings-structure-20261107/index.html)平铺分词词典、阅读记录和本机偏好：历史始终保存，作用范围只影响阅读偏好且保留字段继承；会话接收/翻译在当前会话控制，系统登录启动独立读回。

范围及材质原确认、当前guided与R79历史页完整组合已获用户审核确认、源码实现和真实产品验收分别登记，互不继承。原56项需求、100文件来源、22屏及License保留：[原始来源清单](source-manifest.json)、[原PRD](original/docs/01-prd.md)、[原设计](original/design/index.html)。整理前的叙述见[归档](../archive/documentation-r75/docs/product/README.md)，它不再作为实施入口。
