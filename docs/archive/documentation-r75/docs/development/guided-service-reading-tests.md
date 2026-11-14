# 必要配置与阅读优先：逐层测试、字段供给与功能审计

修订：2026-10-30 · 工作项[design-guided-service-and-reading](../../openspec/changes/design-guided-service-and-reading/proposal.md)。本文件与[配置PRD第10章](../product/model-configuration-prd.md#guided-service-configuration-prd)、[当前阅读PRD](../product/reading-window-session-prd.md#guided-current-reading-prd)、[技术合同5.23](../architecture/japanese-study-and-llm.md#guided-configuration-reading-contract)、[机器目录guided_configuration_design/guided_acceptance](../architecture/model-service-registry-design.json)及[逐字段呈现目录](../architecture/service-configuration-presentation.json)同源。

[新完整候选](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)与[配置入口](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)为`pending_review`。本轮仅文档、候选交互/样式与测试设施；不改产品代码、DDL、OAuth或实际服务/资源状态。GD-01～12全部产品`not_run`，原PA/OT/SS/FL、MC/RW、CSV及153组合保持原内容和状态。

## 逐项计划

| ID | 需求与必须证明的行为 | 本轮设计输入/方法 | 后续真实层 / 产品状态 |
|---|---|---|---|
| GD-01 | FP-04/06/09；AI-CFG-13；RW-17/31/32。独立当前正文卡首屏优先 | 初始、新句、翻译中/失败/关闭、独立重译；核对当前原文/译文/来源、常用动作和辅助预览，不能拿历史列表冒充当前对象 | 真实组件/当前unit与绑定时序/最终包；not_run |
| GD-02 | FP-04/09/12；RW-17/18/20/21。历史/原始提取/Hook在阅读后方默认折叠 | 初始三列表均折叠且DOM顺序在当前卡之后；数量、警告/错误/缺口和查看/恢复入口仍可见，rerender保留用户展开 | 真实组件/事件摘要及数据管线；not_run |
| GD-03 | FP-03/09/12；RW-01/17～23。折叠不丢全集或污染独立状态/预览 | 多页多流、选流前/未选流、重复/追加；分别搜索/过滤/上翻，折叠→推进→展开核对页/滚动/跟随/计数与预览身份；逐数据集完整/过滤导出 | 真实Repository/事件管线/磁盘失败重启/原生预览；not_run |
| GD-04 | FP-06/11；AI-CFG-01/02/14。21HTTP/13协议/4离线精确绑定，品牌多接口明确选 | 对照原registry逐ID、protocol/profile/version与用途集合；品牌同名多接口显示人话说明、须明确确认，建议不授权自动激活或降级 | 目录契约/Rust注册投影/真实组件；not_run |
| GD-05 | FP-06/11；AI-CFG-01/02/04/08。字段分类、条件及默认均有来源 | 每字段id/type/label/classification/condition/source_refs验证；固定地址由目录供给、DeepL套餐/Qwen地域默认按明确选择解析；资料缺口不转嫁用户，模型不伪填 | typed Rust目录/配置领域校验/实际服务；not_run |
| GD-06 | FP-06/11；AI-CFG-01/06/08/11。不适用/内部字段不强填 | 传统接口无LLM模型/温度/模板，本机无URL/Key；protocol/schema/revision/签名/salt/引用只在内部或只读详情；必要字段不能因隐藏省略 | 真实组件/目录能力交集/协议契约；not_run |
| GD-07 | FP-06/11/12；AI-CFG-05/11/15。遮罩秘密和真实授权语义 | 虚构Key/应用密钥输入遮罩；正常draft/saved仅opaque引用，取消/复制/导出不泄露；百度/有道完整typed对，Google v3登录/服务账号及客户端缺口不伪装通用Key | 实际Vault/typed凭据/OAuth PKCE/撤销刷新/实际服务；not_run |
| GD-08 | FP-06/08/11；AI-CFG-03/07～09/11/12/16。高级与学习独立默认按需 | 学习两区初始折叠，分别展开/选择/编辑/测试/应用不串用途；适用模板/三态参数/上下文/网络/运行策略/密钥池仍可达，不显示不适用功能 | 真实组件/Rust用途/独立学习对象与实际请求；not_run |
| GD-09 | FP-04/06/09/12；AI-CFG-03～05/10/12/13/16。已保存资格、版本及启停语义保持 | 未保存/保存/样句/返回不切服务；当前已知失败禁选，HTTP未测试标未验证；显式选用只影响后续句且保持开关/范围，重译保留旧历史，取消迟到隔离 | 绑定/授权代次/Repository/实际传输取消/最终包；not_run |
| GD-10 | FP-06/11/12；AI-CFG-04～06/14。本机未知必要条件仍阻断 | 四引擎分别检查未知系统/API/传统策略、缺包、路径/位数、未实现路线和资源变化；输入目录不标已安装，探测不标翻译通过，不转云端 | 实际平台/合法资源/隔离worker/断网与质量/最终包；not_run |
| GD-11 | FP-06/11/12；AI-CFG-01/02/04/08/11/14。自定义必要地址、认证、模型不隐藏 | 每适用协议切custom，端点/认证/模型/项目/区域按条件显现并校验；本机HTTP认证独立于地址，默认端口也可要求Key；未知条件不当不适用 | 正式组件/Rust配置校验/真实各服务失败恢复；not_run |
| GD-12 | FP-04/06/09/11/12；RW-31～34。可访问样式及证据不外推 | 深/浅、窄窗、长标签/长文、键盘/焦点/Escape、200%分别实拍核对；原批准源/图/token和旧计划不改，新结果绑定准确源码hash | 独立设计审阅/正式组件/原生可访问及最终包四游戏；not_run |

## 字段目录供给与前置

呈现目录是设计层数据源，逐项引用现有21 preset、13 protocol与4 engine；它没有安装/探测/授权/连接能力。UI可由它确定性生成只读设计JavaScript，但不得另写一套各品牌支持/资格列表，更不能将JSON当可执行适配器或批准DTO/DDL。

- 字段分类为`required/conditional/default/advanced/readonly/notapplicable`。`condition.all`全部满足；可选`any`至少满足其一；空all为真。只支持具名字段的eq/in比较，不执行表达式/脚本。用户条件依赖未知且涉及必需项时须显现输入或阻断说明，不按false悄悄跳过；内置固定地址/应用授权资料缺口则单独标前置阻断，不让普通用户补资料。
- 字段类型、标签、choices及source_refs逐项可查；`default_value`只来自已核来源或明确产品默认，`default_by`只对已明确选择且有证据的地域/套餐取值，不自动取第一项。实际账户/服务器模型没有预设默认。`inherit_from`说明语言/策略来自已有用途，不代表服务必然支持。
- 品牌`default_service_id`仅为界面建议；多接口的`interface_confirmation_required`保持明确确认，不因建议、测试失败或目录升级切换协议。已保存资源始终保留自己的精确tuple和旧版本。
- 官方来源可取程度逐项登记：有完整正文、仅官方索引或抓取失败的本地合同依据分别说明。21项内置HTTP固定或已核site/plan地址由目录带入，endpoint仅在明确custom服务器时输入。未核准的其他站点/应用授权资料通过preset_material_blockers保持不可用，不能以要求普通用户填URL代替补齐资料，也不能用example.invalid或样例冒充已配置。默认字段仍须符合实际site、套餐、endpoint规范和当前账户。
- API Key/secret只用虚构设计值；普通资源保存不透明演示引用，真实Key永不写入本轮目录/截图/测试。OAuth按钮只能演示缺项/往返，不能模拟登录就显示真实授权成功；Google项目/区域/IAM及应用客户端前置未满足继续明确待处理；应用内置OAuth未准备好不要求普通用户提供client ID，自有client是明确高级选择。
- 本机安装/系统API/语言对/包/运行路线的实际观察尚未执行；目录、路径存在或人工演示状态不证明可用。未知必要条件仍阻断。本机非大模型与本机HTTP大模型不能混用字段或健康条件。

## 功能审计表

| 呈现问题 | 本轮调整 | 保留的能力与限制 |
|---|---|---|
| 诊断/历史挤占当前正文 | 当前原文/译文为独立首屏，历史/提取/Hook后置默认折叠 | 三数据集完整查询、分页、导出、身份/原生事实/缺口及独立跟随，预览不重建 |
| 按连接/模型/内部字段教用户配置 | 先用途、品牌、明确接口，再显示当前必要项 | 底层实体/引用/版本与Rust权威不变，不把UI简化当缩减业务 |
| 所有协议共用一套Key/模型表单 | 逐服务字段分类和条件；多字段签名/Google授权/本机资源有专用语义 | 原21/13/4完整保留；不把传统接口改成假LLM，不把原生改成兼容Chat |
| 明文引用/版本术语与秘密输入混杂 | 遮罩的用户密钥语义；系统内部维护引用/签名/revision | 安全保存、版本/撤销、导入导出与诊断脱敏不变 |
| 高级与学习表单全摊 | 仅适用内容按需展开，学习两用途独立折叠 | 模板/参数/策略/密钥池仍可达；不串用途、不自动启用、不借模型 |
| 简化后把真正必需项也藏掉 | 自定义地址/认证/模型、地区/项目和未知前置必须显现或阻断 | HTTP未测试与未配置、本机未知健康分别判断，缺项不当成功 |
| 推荐/示例冒充实际配置 | 明确建议与账号资格分开，固定默认由目录供给；缺资料标阻断而非用户代填 | 保存不选用、不自动发样句/游戏文本/下载，旧绑定不被目录更新改写 |
| 200%仅有节点仍在就称适配通过 | 实拍并记录裁切与可达性，各自给证据与限制 | 当前桌面壳Zoom可能裁切；入口/键盘可达不等于完整无裁剪，不提前填GD产品通过 |

## 分层执行与维护

| 层 | 本轮/后续必须执行内容 | 前置与Owner |
|---|---|---|
| 文档与目录静态 | 新GD逐项not_run、字段/来源/条件/21/13/4精确映射、旧JSON所有键值与旧CSV/候选不变，五类负例门禁 | R73完整源快照、catalog与本change实际注册；Root/QA/AP |
| 新候选浏览器 | 实际入口点击GD各项，必要/条件/空/失败/取消/秘密边界、保存选用往返和折叠状态，页面错误/外部请求拒绝 | 现有固定Playwright/Chrome、合法人工样本、独立候选；UI/QA |
| 设计视觉 | 对同主题/字号/数据/scale/窗口状态截图，人工核对当前正文优先、控件/材质与必要操作；200%限制单列 | 独立候选图，不覆盖原批准图/token或放宽阈值；UI/独立审阅 |
| 真实组件与Rust | 正式application-port/DTO及实际字段/资格/用途/绑定/代次/取消/迟到，不在测试复制业务规则 | 后续product工作项、真实组件和领域adapter；M04/M05/M13 |
| 存储/认证/服务/本机 | 真实保存与重启、版本/导出/授权更新撤销、多字段签名/PKCE、真实样句；合法资源/系统与各引擎独立检查 | 已批准结构、实际Vault/账号/额度/可发样句/许可资源/平台；AP/AI/PL/QA |
| 最终正式包 | 当前卡→保存配置→明确选用→后续句/独立重译→折叠历史/提取/Hook→失败/恢复，四游戏分别运行 | 同源签名包、Core/词典/资源与可见桌面；SR/MO/Never7/樱之诗；PL/QA |

静态与浏览器维护建议为`scripts/quality/guided_service_reading.py`、`test_guided_service_reading.py`及`scripts/report/verify_guided_service_reading.cjs`，实际执行命令以本change的`verification.json`注册为准。呈现源归`docs/architecture/service-configuration-presentation.json`，候选及生成设计JS归`docs/design/first-product-v1/candidates/guided-service-reading-20261030/`；旧SS/FL等测试合同不改。

本轮运行在`.build/guided-design-20261030/`下另建run目录，记录实际命令/退出码/断言数、源/目录/生成JS/截图相对路径与hash；失败保留，修复后新记录。Root的代码/文档独审及最终统一verify分别留证。后续真实层另按`client-regression.md`记录同包证据；未执行写`not_run`，外部条件缺失写`blocked`，不提前把计划改绿。

本文不预填设计通过数量；候选控件/布局仍待用户评审。文档、代码和受控浏览器样例不替代真实服务、OAuth、系统API、worker/DLL、原生窗口或四游戏验收。

## 2026-10-31 当前测试与准出同步

本GD原计划/原目录状态保持，最新附加合同为[current-product-test-coverage.md](current-product-test-coverage.md)及[70来源机器登记](current-product-test-coverage.json)。25项是21 HTTP预设与4离线引擎，不是25 HTTP preset；13协议与精确版本/用途不可因呈现简化改写。

现有 `verify_guided_service_reading.cjs`的GUI01～08回归是可追溯的设计设施：代理独立保存；学习用途温度三态/模板；新资源与既有绑定隔离；主阅读复制/重译；stale与资源任务独立；池观察不清失败；传统预览无LLM prompt；翻译上下文范围与保存/应用隔离。新机器登记逐项补需求、Owner/M-C、当前设计锚点、真实层、前置与观察证据；设计演示通过不填写产品pass。

准出同时要求旧153与新576观察组合；新证据通过原release对象的 `current_product_coverage` path/hash链接，不能把新ID写入旧153集合。每项按代码注册真实能力核command、源码/包/nonce、真实日志、断言、时间与证据。权限、实际Vault/服务、存储重启/故障、200%实拍、四游戏各五不同状态及Never7三页独立核对。当前真实能力缺口继续not_run/blocked，静态合同/守卫通过不代最终包。

当前补充准出仅覆盖既定macOS arm64+CrossOver同一包：576必需观察；原OT-08/OT-16的J北京/金山Windows原机4项另列 `separate_platform_qualifications`，保持not_run、独立环境/ABI/合法资源/Windows工件，不能当Mac包通过或从目录删掉。机器代码限定这4项单列，不允许JSON任意缩减当前scope；未来Windows发布需独立范围与真实准出。

OT-16当前正式包层按4引擎×4游戏显式展开16项，不能由独立worker检查和四款聚合game结果互相顶替。每项冻结engine ID、contract revision、backend、Mac/CrossOver runtime，并要求精确该引擎/游戏/包/nonce的逐状态实际译文、合法资源版本/文件hash/许可证据及独立质量审阅；SR非日语分支仍如实记录。四款至少各五不同游戏/客户端画面，Never7至少三页。原Windows4资格仍在当前scope之外独立not_run。

OT-16中SR既有中文版只按真实同语种原文、直通、非日语降级或实际非日语翻译分别记录；直通/未调用引擎的证据只能通过非日语分支检查，不得记日语译文质量。其他三款日文游戏必须有该引擎真实调用、原译文及独立日语质量结论。每引擎/游戏的资源状态必须ready、已授权且route支持；缺授权/未知/不支持仍blocked，不因直通、显示中文或其他引擎通过而绕过。
