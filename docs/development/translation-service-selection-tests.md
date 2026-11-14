# 翻译服务选择：逐层测试与依赖

修订：2026-10-30 · 本文件定义SS-01～12独立增量，机器来源为[selection_acceptance](../architecture/model-service-registry-design.json)。对应[PRD第8章](../product/model-configuration-prd.md#translation-service-selection-prd)、[技术合同5.19～5.21](../architecture/japanese-study-and-llm.md#translation-service-selection-contract)与[完整候选稿](../design/first-product-v1/candidates/translation-service-selection-20261030/index.html?scenario=reading)。新交互pending_review；原R71候选、PA12/OT16、MC22、原矩阵与153组合保留。

本轮可执行文档/候选HTML检查与Playwright只证明设计层。下表“产品状态”全部not_run；未来真实层缺外部条件时在独立运行记录写blocked，不能把计划改为通过或用设计样例代替产品输出。

## 逐项计划与数据前置

| ID | 要证明的行为 | 设计模拟输入 | 后续真实层 / 产品状态 |
|---|---|---|---|
| SS-01 | 当前会话浅层入口与HUD复用同一选择器；切换后同源更新 | 已保存云端、本机、LLM翻译项及两个窗口入口 | 真实组件/多窗Rust投影/原生；not_run |
| SS-02 | 只读已添加、已保存、字段合法且支持translation的资源；未添加模板只在添加服务 | 空目录、未保存草稿、合法保存版本、同名不同资源 | 领域可选性/配置投影/真实组件；not_run |
| SS-03 | 未配置、字段缺失、凭据不可用、当前版本已知失败和停用不可选；原因可恢复 | 对象/版本明确的缺项、失败、撤权与停用状态 | Rust校验/真实Vault和错误回执；not_run |
| SS-04 | HTTP已配置未测试可明确选用且仍标未验证；旧测试过期不等于未配置 | 合法未测配置、旧版本通过、新版本未测 | 领域版本/真实样句/服务错误；not_run |
| SS-05 | 本机沿既有条件、资源/语言对与运行route健康门禁；未知不放行 | Apple必要条件未知/缺包，Argos路线未实现，DLL条件失败 | 真实平台/资源/worker与各环境；not_run |
| SS-06 | 精读-only配置不进翻译列表；显式templatePurpose与能力共同过滤 | 翻译/grammar-only同品牌同名、translation不支持或未知 | Rust目录/用途/导入越权负例；not_run |
| SS-07 | 保存资源和测试不改变current；选择引用已保存不可变版本 | 编辑未保存、保存新revision、旧绑定仍引用旧revision | 真实配置服务/存储/绑定依赖；not_run |
| SS-08 | 选中即激活新绑定只对后续句；启停保持，当前句重译独立且旧结果保留 | 已开启/关闭两态，当前句与下一句，独立重译 | Rust调度/真实API/正式包；not_run |
| SS-09 | 全局默认、当前游戏覆盖和恢复继承来源明确，修改互不串用 | 两游戏、全局项、单游戏覆盖与取消 | 领域scope/持久化/配置档隔离；not_run |
| SS-10 | 取消不改绑定；切换使旧任务失效，迟到结果不覆盖新句/缓存，历史保留来源 | 在途旧任务、取消、迟到分片、重开选择器 | Rust竞态/真实传输取消/历史与缓存；not_run |
| SS-11 | 主翻译一个、对照有序去重；同一配置不能同时充当主/对照，单项失败不取消他项 | 主/对照互换、重复点击、同名不同ID、一个失败 | 领域多引擎/并发预算/正式包；not_run |
| SS-12 | 去配置携带来源、scope/game、用途和目标；保存/取消后返回，不自动选用或发文本 | HUD与主窗来源、配置草稿、保存新项与取消 | 真实路由/多窗/配置恢复/同包；not_run |

## 层次、执行方式与限制

| 层 | 必须执行的验证 | 必须具备的输入与负责人 |
|---|---|---|
| 文档静态 | 13HTTP/21预设及4离线候选逐对象不变、旧PA/OT/矩阵不变；SS条目/状态/锚点和所有权范围一致 | 变更前冻结来源、新PRD/技术/目录与候选manifest；AP/QA |
| 候选设计 | 从真实浅层/HUD入口点击，保存→返回→明确选择，禁用原因、范围、去重、取消与独立重译；浏览器控制台/异常请求检查 | 本轮候选页面、固定Playwright/Chrome与合法人工演示数据；UI/QA |
| 视觉评审 | 暗/浅、窄窗、长名称、200%、键盘/焦点/Escape与禁用；新截图独立留存并与既有材质核对 | 相同视口/字体/数据/scale、确认材质；UI/QA；新界面仍待用户评审 |
| 真实组件 | 通过正式application-port读取投影/发送命令，不在测试里复制可选性规则；主窗/HUD一致更新与错误恢复 | 后续实现、生成DTO与真实组件adapter；UI/AP |
| Rust领域与时序 | 选择资格二次校验、预期revision冲突、不可变资源引用、scope、purpose enabled不变、主/对照去重、旧epoch/取消/迟到隔离 | 真实配置/绑定/调度代码，确定性时钟与可控任务；AP/AI |
| 存储与迁移 | 真实保存revision/旧绑定/历史可追溯、重启恢复、配置档隔离；若需新字段先另审DDL/codec/迁移 | 已批准结构与真实Repository、冻结旧profile副本、PRE-14矩阵；AP/QA；本轮不改DDL |
| 实际服务与本机 | 合法HTTP未测首次选用后的真实日中请求、失败恢复、凭据撤销；本机按OT条件实测资源/route/worker | 可发送样句、实际账号/额度/网络或合法本机安装/语言包及相应平台；AI/PL/QA |
| 正式包端到端 | 主窗→HUD→选择→后续句→当前句重译→历史→取消/停用/恢复，四游戏分别记录 | 同源签名包、Core/语言/引擎资源身份、可见桌面、SR/MO/Never7/樱之诗；PL/QA |

真实HTTP未测试用例需要可发请求的实际配置，不能给它预填测试通过；已知失败用例需要绑定当前版本的实际失败或明确故障注入。选择失败/冲突应保留原绑定，不能改成自动挑第一项。HTTP连通性未知与本机必需资源/执行route未知是不同前提，不共用一个“未知即可选”开关。

## 维护位置与运行证据

- 计划权威：本文件、model-service-registry-design.json的service_selection_design/selection_acceptance；原MC/OT/PA及计划CSV保持原状态。
- 候选源码：docs/design/first-product-v1/candidates/translation-service-selection-20261030/；R71 local-offline-20261030目录不覆盖。候选list/selection JavaScript API仅模拟已保存投影，正式产品仍由Rust持业务权威。
- 本轮静态守卫维护在scripts/quality/translation_service_selection.py、test_translation_service_selection.py，浏览器旅程维护在scripts/report/verify_translation_service_selection.cjs；执行入口以本change的verification.json实际注册命令为准。检查回执须记录命令、退出码、断言数、候选source hash、截图及相对路径。未注册或没有实际执行时不得填写通过。
- 后续生产测试建议复用crates/app-core/tests的model configuration/binding/runtime套件、crates/app-infra/tests的storage/provider套件及apps/desktop/tests的model-config/reading真实组件；这些是维护归属建议，不声称本轮已建立生产adapter。
- 设计运行记录另建.build/translation-service-selection-20261030/下独立run目录；截图不写原previews或批准目录。正式产品按client-regression.md在.build/client-regression/逐run记录源码/包/资源/环境、实际操作和证据hash。
- 未配置/缺账号/无合法DLL或语言包/平台不支持时，仅受影响实际层写blocked并保留原因；未执行为not_run。设计guard、合成故障、文档hash和浏览器图片不能提升实际服务、迁移、原生或同包四游戏结果。

## 本轮结果的填写约束

本文不提前填写设计或产品通过数字。实际静态和Playwright回执由本次执行生成，失败保留并修复后另跑新记录；历史成功不追认新源码。用户确认日常选择任务模型，只批准该决策方向；新候选控件、截图与细粒度契约仍独立待评审。
