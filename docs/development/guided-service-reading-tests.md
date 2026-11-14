# 必要配置与阅读优先：逐层测试、字段供给与功能审计

当前guided完整源版本已由用户明确审核通过，身份见[源版批准记录](design-approval-guided-20261031.json)；其他页面按[全页与状态闭包](all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

设计来源工作项[design-guided-service-and-reading](../../openspec/changes/design-guided-service-and-reading/proposal.md)。本文件与[配置PRD第10章](../product/model-configuration-prd.md#guided-service-configuration-prd)、[当前阅读PRD](../product/reading-window-session-prd.md#guided-current-reading-prd)、[技术合同5.23](../architecture/japanese-study-and-llm.md#guided-configuration-reading-contract)、[机器目录guided_configuration_design/guided_acceptance](../architecture/model-service-registry-design.json)及[逐字段呈现目录](../architecture/service-configuration-presentation.json)同源。

[新完整候选](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)与[配置入口](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)源版本已获用户审核确认。这是设计目标与逐层测试合同，不授予产品代码、DDL、OAuth或实际服务/资源通过状态。GD-01～12全部产品`not_run`，原PA/OT/SS/FL、MC/RW、CSV及153组合保持原内容和状态。

## 逐项计划

| ID | 需求与必须证明的行为 | 设计输入/方法 | 后续真实层 / 产品状态 |
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
- API Key/secret只用虚构设计值；普通资源保存不透明演示引用，真实Key永不写入设计目录/截图/测试。OAuth按钮只能演示缺项/往返，不能模拟登录就显示真实授权成功；Google项目/区域/IAM及应用客户端前置未满足继续明确待处理；应用内置OAuth未准备好不要求普通用户提供client ID，自有client是明确高级选择。
- 本机安装/系统API/语言对/包/运行路线的实际观察尚未执行；目录、路径存在或人工演示状态不证明可用。未知必要条件仍阻断。本机非大模型与本机HTTP大模型不能混用字段或健康条件。

## 目标行为与保留能力

| 呈现问题 | 当前目标 | 保留的能力与限制 |
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

| 层 | 必须执行内容 | 前置与Owner |
|---|---|---|
| 文档与目录静态 | GD逐项not_run、字段/来源/条件/21/13/4精确映射、原JSON/CSV定义及候选身份、五类负例门禁 | 当前源闭包、catalog与适用change实际注册；Root/QA/AP |
| 新候选浏览器 | 实际入口点击GD各项，必要/条件/空/失败/取消/秘密边界、保存选用往返和折叠状态，页面错误/外部请求拒绝 | 现有固定Playwright/Chrome、合法人工样本、独立候选；UI/QA |
| 设计视觉 | 对同主题/字号/数据/scale/窗口状态截图，人工核对当前正文优先、控件/材质与必要操作；200%限制单列 | 独立候选图，不覆盖原批准图/token或放宽阈值；UI/独立审阅 |
| 真实组件与Rust | 正式application-port/DTO及实际字段/资格/用途/绑定/代次/取消/迟到，不在测试复制业务规则 | 后续product工作项、真实组件和领域adapter；M04/M05/M13 |
| 存储/认证/服务/本机 | 真实保存与重启、版本/导出/授权更新撤销、多字段签名/PKCE、真实样句；合法资源/系统与各引擎独立检查 | 已批准结构、实际Vault/账号/额度/可发样句/许可资源/平台；AP/AI/PL/QA |
| 最终正式包 | 当前卡→保存配置→明确选用→后续句/独立重译→折叠历史/提取/Hook→失败/恢复，四游戏分别运行 | 同源签名包、Core/词典/资源与可见桌面；SR/MO/Never7/樱之诗；PL/QA |

## 累计覆盖与真实执行边界

原128定义/153组合、原111条CR定义、RW34六层及真实存储专项继续独立强制；当前PA12+OT16+SS12+FL10+GD12+GUI8累计70来源/576观察另由[current覆盖](current-product-test-coverage.md)和[机器登记](current-product-test-coverage.json)验证，25服务精确为21HTTP+4离线，协议13种。GUI8保留代理、学习温度/模板、资源草稿隔离、主卡复制/重译、stale独立任务、池观察不清失败、传统预览及上下文保存/应用隔离。GD权威验收编号与设计runner旅程编号语义不同，不用编号相同推断覆盖。

[当前设计对齐合同](current-design-alignment.json)固定guided与R79完整页面组合、适用状态/61源以及需求、Owner、生产组件路径；候选原快照标记、当前源版用户确认、独立参照与批准catalogue分开。开发产品UI变更必须有真正支持当前目标及所需状态的代码adapter，未知desktop/Rust/native影响保守要求完整集合。旧v4只能回归历史，原型互比、旧PNG重标和source/component冒native均不能满足当前验收。

实际React和正式包分别执行，adapter必须有分离的execute、inspect和独立recompute，不能由JSON自报能力。当前有4态独立批准参照与相应有限真实React执行器，其余状态和完整Native能力仍缺，原像素和有效render失败保留；完整设计源已获用户审核确认，产品对齐及发布仍blocked。静态合同或设计交互检查通过不提升这些状态；GD-01～12计划行保持原样not_run。

正式包逐款完成SR/MO/Never7/樱之诗各5个不同游戏/客户端配对，Never7至少3页；权限、Vault、SQLite、主题/窄窗/200%按独立变体留真实证据。OT-16逐4引擎×4游戏展开16项，绑定准确引擎/contract/backend/runtime/包/nonce，核合法资源、实际逐状态结果及独立质量。SR既有中文版的直通/非日语降级不计日语译文质量，其他三款须实际日语翻译；未知、未授权或不支持route继续blocked。

当前576项仅属macOS arm64+CrossOver同包范围。OT-08/OT-16×J北京/金山共4项Windows原机资格独立保留not_run及环境、同位数worker/DLL ABI、合法资源与Windows工件，不能删除、混入Mac通过或复用Mac包hash。

## 维护与运行记录

设计静态/浏览器工具为 `scripts/quality/guided_service_reading.py`、`test_guided_service_reading.py`及 `scripts/report/verify_guided_service_reading.cjs`，执行命令以适用change登记和实际回执为准；当前设计门禁另为 `current_design_alignment.py/test_current_design_alignment.py`。呈现源及候选路径见本文入口，不另写字段支持表。

每次执行另建 `.build/<run_id>/`，记录命令/退出码/断言、精确源/目录/包/nonce、截图及hash、独立审阅与限制。原153、新576及完整页面状态证据分别按[发布格式](release-execution-evidence.md)验证；原计划不回填。未执行not_run，外部条件不足blocked，实际失败留证并修复后重跑；设计样例不替代服务、OAuth、系统API、worker/DLL、原生或四游戏。

旧正文原字节保存在[文档归档](../archive/documentation-r75/docs/development/guided-service-reading-tests.md)，只供追溯，不作为现行执行入口。
